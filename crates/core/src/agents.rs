//! Connecting AI agents to the build assistant's MCP server. Each agent keeps its MCP servers in
//! its own file and format, checked against their docs on 2026-10-23. Adding ours keeps the rest
//! of the file as it was.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const SERVER_NAME: &str = "exile-companion";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentId {
    ClaudeDesktop,
    ClaudeCode,
    Cursor,
    VsCode,
    Codex,
}

pub const AGENTS: [AgentId; 5] = [
    AgentId::ClaudeDesktop,
    AgentId::ClaudeCode,
    AgentId::Cursor,
    AgentId::VsCode,
    AgentId::Codex,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Servers under a top-level `mcpServers` object, each with a `command`.
    Json,
    /// `[mcp_servers.<name>]` tables in Codex's config.toml.
    CodexToml,
}

/// The folders the config files hang off. `app_data` is the roaming folder on Windows
/// (%APPDATA%), Application Support on macOS and ~/.config elsewhere.
#[derive(Debug, Clone)]
pub struct Dirs {
    pub home: PathBuf,
    pub app_data: PathBuf,
    /// $COPILOT_HOME and $CODEX_HOME, when set.
    pub copilot_home: Option<PathBuf>,
    pub codex_home: Option<PathBuf>,
}

impl AgentId {
    pub fn name(self) -> &'static str {
        match self {
            AgentId::ClaudeDesktop => "Claude Desktop",
            AgentId::ClaudeCode => "Claude Code",
            AgentId::Cursor => "Cursor",
            AgentId::VsCode => "VS Code (Copilot)",
            AgentId::Codex => "Codex",
        }
    }

    pub fn format(self) -> Format {
        match self {
            AgentId::Codex => Format::CodexToml,
            _ => Format::Json,
        }
    }

    pub fn config_file(self, d: &Dirs) -> PathBuf {
        match self {
            AgentId::ClaudeDesktop => d.app_data.join("Claude").join("claude_desktop_config.json"),
            // User-scope servers live in ~/.claude.json, next to the rest of Claude Code's state.
            AgentId::ClaudeCode => d.home.join(".claude.json"),
            AgentId::Cursor => d.home.join(".cursor").join("mcp.json"),
            // VS Code reads Copilot's global file as well as its own profile file.
            AgentId::VsCode => d
                .copilot_home
                .clone()
                .unwrap_or_else(|| d.home.join(".copilot"))
                .join("mcp-config.json"),
            AgentId::Codex => d
                .codex_home
                .clone()
                .unwrap_or_else(|| d.home.join(".codex"))
                .join("config.toml"),
        }
    }

    /// A folder that exists once the agent has run on this machine.
    pub fn marker(self, d: &Dirs) -> PathBuf {
        match self {
            AgentId::ClaudeDesktop => d.app_data.join("Claude"),
            AgentId::ClaudeCode => d.home.join(".claude"),
            AgentId::Cursor => d.home.join(".cursor"),
            AgentId::VsCode => d.app_data.join("Code"),
            AgentId::Codex => self
                .config_file(d)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum Status {
    NotFound,
    NotConnected,
    Connected,
    /// Our server is there but starts another exile-companion-mcp, an older install or a build.
    OtherCopy {
        command: String,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AgentError {
    #[error("the config file isn't valid {format}: {reason}")]
    Unreadable {
        format: &'static str,
        reason: String,
    },
    #[error("the config file's {0} isn't a table of servers")]
    WrongShape(&'static str),
}

pub fn status(installed: bool, config: Option<&str>, format: Format, command: &str) -> Status {
    match config.and_then(|text| server_command(format, text)) {
        Some(c) if same_command(&c, command) => Status::Connected,
        Some(c) => Status::OtherCopy { command: c },
        None if installed => Status::NotConnected,
        None => Status::NotFound,
    }
}

fn same_command(a: &str, b: &str) -> bool {
    // Windows paths compare without case, and agents write either slash.
    let norm = |s: &str| s.replace('\\', "/").to_lowercase();
    norm(a) == norm(b)
}

/// The command our server entry starts, if the config has one.
pub fn server_command(format: Format, text: &str) -> Option<String> {
    match format {
        Format::Json => {
            let v: Value = serde_json::from_str(text).ok()?;
            v.get("mcpServers")?
                .get(SERVER_NAME)?
                .get("command")?
                .as_str()
                .map(str::to_string)
        }
        Format::CodexToml => {
            let doc: toml_edit::DocumentMut = text.parse().ok()?;
            doc.get("mcp_servers")?
                .get(SERVER_NAME)?
                .get("command")?
                .as_str()
                .map(str::to_string)
        }
    }
}

/// The config with our server added or pointed at `command`. Everything else stays as it was.
pub fn with_server(
    format: Format,
    existing: Option<&str>,
    command: &str,
) -> Result<String, AgentError> {
    let existing = existing.filter(|t| !t.trim().is_empty());
    match format {
        Format::Json => {
            let mut root: Value = match existing {
                Some(text) => serde_json::from_str(text).map_err(|e| AgentError::Unreadable {
                    format: "JSON",
                    reason: e.to_string(),
                })?,
                None => Value::Object(Map::new()),
            };
            let root_obj = root
                .as_object_mut()
                .ok_or(AgentError::WrongShape("top level"))?;
            let servers = root_obj
                .entry("mcpServers")
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .ok_or(AgentError::WrongShape("mcpServers"))?;
            let mut entry = Map::new();
            entry.insert("command".into(), Value::String(command.into()));
            entry.insert("args".into(), Value::Array(Vec::new()));
            servers.insert(SERVER_NAME.into(), Value::Object(entry));
            let mut out = serde_json::to_string_pretty(&root).expect("a JSON value serializes");
            out.push('\n');
            Ok(out)
        }
        Format::CodexToml => {
            let mut doc: toml_edit::DocumentMut =
                existing
                    .unwrap_or("")
                    .parse()
                    .map_err(|e: toml_edit::TomlError| AgentError::Unreadable {
                        format: "TOML",
                        reason: e.to_string(),
                    })?;
            let servers = doc
                .entry("mcp_servers")
                .or_insert(toml_edit::table())
                .as_table_mut()
                .ok_or(AgentError::WrongShape("mcp_servers"))?;
            // Only [mcp_servers.exile-companion] shows up, not an empty [mcp_servers] header.
            servers.set_implicit(true);
            let mut entry = toml_edit::Table::new();
            entry.insert("command", toml_edit::value(command));
            servers.insert(SERVER_NAME, toml_edit::Item::Table(entry));
            Ok(doc.to_string())
        }
    }
}

/// What to paste into an agent this app doesn't know, in the shape most of them read.
pub fn snippet(command: &str) -> String {
    with_server(Format::Json, None, command).expect("an empty config takes the server")
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = r"C:\Users\me\AppData\Local\Exile Companion\exile-companion-mcp.exe";

    #[test]
    fn adds_the_server_to_a_json_config_and_keeps_the_rest() {
        let before = r#"{
  "globalShortcut": "Ctrl+Space",
  "mcpServers": {
    "filesystem": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem"] }
  }
}"#;
        let after = with_server(Format::Json, Some(before), EXE).unwrap();
        let v: Value = serde_json::from_str(&after).unwrap();
        assert_eq!(v["globalShortcut"], "Ctrl+Space");
        assert_eq!(v["mcpServers"]["filesystem"]["command"], "npx");
        assert_eq!(v["mcpServers"][SERVER_NAME]["command"], EXE);
        // Key order survives, so the player's file doesn't get reshuffled.
        assert!(after.find("globalShortcut").unwrap() < after.find("mcpServers").unwrap());
        assert_eq!(server_command(Format::Json, &after).as_deref(), Some(EXE));
    }

    #[test]
    fn creates_a_json_config_and_repoints_an_old_entry() {
        let fresh = with_server(Format::Json, None, "old.exe").unwrap();
        let moved = with_server(Format::Json, Some(&fresh), EXE).unwrap();
        assert_eq!(server_command(Format::Json, &moved).as_deref(), Some(EXE));
        assert_eq!(
            with_server(Format::Json, Some("  \n"), EXE).unwrap(),
            snippet(EXE)
        );
    }

    #[test]
    fn refuses_a_config_it_cannot_read() {
        assert!(matches!(
            with_server(Format::Json, Some("{ broken"), EXE),
            Err(AgentError::Unreadable { format: "JSON", .. })
        ));
        assert_eq!(
            with_server(Format::Json, Some(r#"{"mcpServers": []}"#), EXE),
            Err(AgentError::WrongShape("mcpServers"))
        );
    }

    #[test]
    fn adds_the_server_to_codex_toml_and_keeps_comments() {
        let before = "# my settings\nmodel = \"gpt-5\"\n\n[mcp_servers.context7]\ncommand = \"npx\"\nargs = [\"-y\", \"@upstash/context7-mcp\"]\n";
        let after = with_server(Format::CodexToml, Some(before), EXE).unwrap();
        assert!(
            after.starts_with("# my settings\nmodel = \"gpt-5\""),
            "{after}"
        );
        assert!(after.contains("[mcp_servers.context7]"));
        assert!(after.contains("[mcp_servers.exile-companion]"), "{after}");
        assert!(!after.contains("[mcp_servers]\n"), "{after}");
        assert_eq!(
            server_command(Format::CodexToml, &after).as_deref(),
            Some(EXE)
        );
    }

    #[test]
    fn tells_connected_from_another_copy() {
        let cfg = with_server(Format::Json, None, EXE).unwrap();
        let other = cfg.replace("Local", "Roaming");
        let lower =
            with_server(Format::Json, None, &EXE.to_lowercase().replace('\\', "/")).unwrap();
        assert_eq!(
            status(true, Some(&cfg), Format::Json, EXE),
            Status::Connected
        );
        assert_eq!(
            status(true, Some(&lower), Format::Json, EXE),
            Status::Connected
        );
        assert!(matches!(
            status(true, Some(&other), Format::Json, EXE),
            Status::OtherCopy { .. }
        ));
        assert_eq!(
            status(true, Some("{}"), Format::Json, EXE),
            Status::NotConnected
        );
        assert_eq!(status(false, None, Format::Json, EXE), Status::NotFound);
    }

    #[test]
    fn puts_each_agent_where_its_docs_say() {
        let d = Dirs {
            home: PathBuf::from("/h"),
            app_data: PathBuf::from("/a"),
            copilot_home: None,
            codex_home: Some(PathBuf::from("/c")),
        };
        assert_eq!(
            AgentId::ClaudeDesktop.config_file(&d),
            PathBuf::from("/a/Claude/claude_desktop_config.json")
        );
        assert_eq!(
            AgentId::ClaudeCode.config_file(&d),
            PathBuf::from("/h/.claude.json")
        );
        assert_eq!(
            AgentId::Cursor.config_file(&d),
            PathBuf::from("/h/.cursor/mcp.json")
        );
        assert_eq!(
            AgentId::VsCode.config_file(&d),
            PathBuf::from("/h/.copilot/mcp-config.json")
        );
        assert_eq!(
            AgentId::Codex.config_file(&d),
            PathBuf::from("/c/config.toml")
        );
    }
}
