//! Connects AI agents on this machine to the build assistant's MCP server, which ships next to the
//! app. The file formats live in exile_core::agents.

use std::path::{Path, PathBuf};

use exile_core::agents::{self, AgentId, Dirs, Format, Status};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentView {
    pub id: AgentId,
    pub name: &'static str,
    pub status: Status,
    pub config: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantView {
    /// The MCP server binary. None in a build without it, such as `pnpm tauri dev` before the
    /// sidecars are built.
    pub server: Option<String>,
    pub agents: Vec<AgentView>,
}

fn env(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn dirs() -> Option<Dirs> {
    let home = env("USERPROFILE").or_else(|| env("HOME"))?;
    let app_data = if cfg!(windows) {
        env("APPDATA")?
    } else if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else {
        env("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"))
    };
    Some(Dirs {
        home,
        app_data,
        copilot_home: env("COPILOT_HOME"),
        codex_home: env("CODEX_HOME"),
    })
}

pub fn server_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let server = exe.parent()?.join(format!(
        "exile-companion-mcp{}",
        std::env::consts::EXE_SUFFIX
    ));
    server.is_file().then_some(server)
}

pub fn view() -> AssistantView {
    let server = server_path().map(|p| p.to_string_lossy().into_owned());
    let agents = match dirs() {
        Some(d) => agents::AGENTS
            .iter()
            .map(|&id| {
                let file = id.config_file(&d);
                let text = std::fs::read_to_string(&file).ok();
                let installed = id.marker(&d).is_dir() || file.is_file();
                AgentView {
                    id,
                    name: id.name(),
                    status: agents::status(
                        installed,
                        text.as_deref(),
                        id.format(),
                        server.as_deref().unwrap_or(""),
                    ),
                    config: file.to_string_lossy().into_owned(),
                }
            })
            .collect(),
        None => Vec::new(),
    };
    AssistantView { server, agents }
}

pub fn connect(id: AgentId) -> Result<AssistantView, String> {
    let server =
        server_path().ok_or("The assistant isn't installed with this build of Exile Companion.")?;
    let server = server.to_string_lossy().into_owned();
    let d = dirs().ok_or("Couldn't find your home folder.")?;
    // Claude Code writes ~/.claude.json all the time, so its own command is the safer way in.
    if id == AgentId::ClaudeCode && claude_code_cli(&server) {
        return Ok(view());
    }
    write_config(&id.config_file(&d), id.format(), &server)?;
    log::info!("connected {} to the assistant", id.name());
    Ok(view())
}

pub fn snippet() -> Option<String> {
    server_path().map(|p| agents::snippet(&p.to_string_lossy()))
}

fn write_config(file: &Path, format: Format, server: &str) -> Result<(), String> {
    let existing = match std::fs::read_to_string(file) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("Couldn't read {}: {e}", file.display())),
    };
    let updated = agents::with_server(format, existing.as_deref(), server)
        .map_err(|e| format!("Left {} as it was, {e}.", file.display()))?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    }
    // One copy of the file as it was before we first touched it.
    let backup = file.with_extension(format!(
        "{}.before-exile-companion",
        file.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    if existing.is_some() && !backup.exists() {
        let _ = std::fs::copy(file, &backup);
    }
    // Written next to the file and renamed over it, so a crash never leaves half a config.
    let tmp = file.with_extension("exile-companion.tmp");
    std::fs::write(&tmp, updated).map_err(|e| format!("Couldn't write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, file).map_err(|e| format!("Couldn't write {}: {e}", file.display()))
}

/// `claude mcp add` at user scope. False when the claude command isn't there or refuses.
fn claude_code_cli(server: &str) -> bool {
    let run = |args: &[&str]| -> bool {
        let mut cmd = if cfg!(windows) {
            // npm installs claude.cmd, which only cmd can start.
            let mut c = std::process::Command::new("cmd");
            c.arg("/C").arg("claude");
            c
        } else {
            std::process::Command::new("claude")
        };
        cmd.args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.status().map(|s| s.success()).unwrap_or(false)
    };
    // add refuses a name that's already there, so an old entry goes first.
    let _ = run(&["mcp", "remove", "--scope", "user", agents::SERVER_NAME]);
    run(&[
        "mcp",
        "add",
        "--transport",
        "stdio",
        "--scope",
        "user",
        agents::SERVER_NAME,
        "--",
        server,
    ])
}
