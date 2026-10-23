//! Talks MCP to the server binary over stdio, as Claude Desktop does.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/pob-engine/tests/fixtures/ranger-caustic-arrow.xml"
);
const GUIDE: &str =
    include_str!("../../../crates/pob-engine/tests/fixtures/ranger-caustic-arrow-guide.xml");

struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    id: u64,
}

impl Client {
    fn start(args: &[&str]) -> Self {
        Self::spawn(Command::new(env!("CARGO_BIN_EXE_exile-companion-mcp")).args(args))
    }

    /// With the home and app data folders moved, so the app's saved files aren't found.
    fn start_env(args: &[&str], home: &std::path::Path) -> Self {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_exile-companion-mcp"));
        cmd.args(args)
            .env("HOME", home)
            .env("USERPROFILE", home)
            .env("APPDATA", home)
            .env("XDG_CONFIG_HOME", home);
        Self::spawn(&mut cmd)
    }

    fn spawn(cmd: &mut Command) -> Self {
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut c = Self {
            child,
            stdin,
            stdout,
            id: 0,
        };
        let init = c.request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }),
        );
        assert_eq!(init["serverInfo"]["name"], "exile-companion");
        c.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        c
    }

    fn send(&mut self, msg: Value) {
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        self.send(json!({ "jsonrpc": "2.0", "id": self.id, "method": method, "params": params }));
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let resp: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["id"], self.id, "{resp}");
        resp["result"].clone()
    }

    /// The tool's text, and whether the server marked it as an error.
    fn tool(&mut self, name: &str, arguments: Value) -> (String, bool) {
        let r = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        let text = r["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        (text, r["isError"] == true)
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[test]
fn lists_its_tools() {
    let mut c = Client::start(&[]);
    let tools = c.request("tools/list", json!({}));
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for name in [
        "build_overview",
        "build_details",
        "what_if",
        "compare_item",
        "compare_with_guide",
        "find_guides",
        "wiki_search",
        "wiki_page",
    ] {
        assert!(names.contains(&name), "{name} missing from {names:?}");
    }
}

#[test]
fn says_what_is_missing_instead_of_failing() {
    let empty = tempfile::tempdir().unwrap();
    let pob = empty.path().to_str().unwrap();
    let mut c = Client::start(&["--engine", "/nowhere/exile-pob-engine", "--pob", pob]);
    let (text, is_error) = c.tool("build_overview", json!({}));
    assert!(is_error);
    assert!(text.contains("exile-pob-engine"), "{text}");
}

// Needs a Path of Building copy (EXILE_POB_DIR) and the engine binary built next to this one,
// which `cargo test -p exile-pob-engine -p exile-companion-mcp` does.
#[test]
fn answers_about_a_build_with_pob() {
    let Some(pob) = std::env::var_os("EXILE_POB_DIR") else {
        eprintln!("EXILE_POB_DIR is not set, skipping");
        return;
    };
    let engine = PathBuf::from(env!("CARGO_BIN_EXE_exile-companion-mcp"))
        .with_file_name(format!("exile-pob-engine{}", std::env::consts::EXE_SUFFIX));
    if !engine.is_file() {
        eprintln!("{} isn't built, skipping", engine.display());
        return;
    }
    let mut c = Client::start(&[
        "--engine",
        engine.to_str().unwrap(),
        "--pob",
        pob.to_str().unwrap(),
        "--build",
        FIXTURE,
    ]);
    let (text, is_error) = c.tool("build_overview", json!({}));
    assert!(!is_error, "{text}");
    let overview: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(overview["main_skill"], "Caustic Arrow");

    let (text, is_error) = c.tool("what_if", json!({ "changes": [{ "level": 30 }] }));
    assert!(!is_error, "{text}");
    let r: Value = serde_json::from_str(&text).unwrap();
    assert!(r["life"].as_f64().unwrap() > 0.0, "{r}");

    let (text, is_error) = c.tool("compare_with_guide", json!({ "guide": GUIDE }));
    assert!(!is_error, "{text}");
    let cmp: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(cmp["passives"]["missing"][0]["node"]["name"], "Herbalism");
    // The same guide again is the same reference, not a second one.
    let (text, _) = c.tool("passive_value", json!({ "guide": GUIDE, "limit": 3 }));
    let power: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(power["reachable"][0]["node"]["name"], "Herbalism");

    let (text, is_error) = c.tool("export_build", json!({}));
    assert!(!is_error, "{text}");
    let code: Value = serde_json::from_str(&text).unwrap();
    assert!(code["code"].as_str().unwrap().starts_with('e'));
}

// A new player has no build in the app. Loading one in the chat has to work then.
#[test]
fn loads_a_build_when_the_app_has_none() {
    let Some(pob) = std::env::var_os("EXILE_POB_DIR") else {
        eprintln!("EXILE_POB_DIR is not set, skipping");
        return;
    };
    let engine = PathBuf::from(env!("CARGO_BIN_EXE_exile-companion-mcp"))
        .with_file_name(format!("exile-pob-engine{}", std::env::consts::EXE_SUFFIX));
    if !engine.is_file() {
        eprintln!("{} isn't built, skipping", engine.display());
        return;
    }
    // HOME and APPDATA point at an empty folder, so there's no saved build to fall back on.
    let empty = tempfile::tempdir().unwrap();
    let mut c = Client::start_env(
        &[
            "--engine",
            engine.to_str().unwrap(),
            "--pob",
            pob.to_str().unwrap(),
        ],
        empty.path(),
    );
    let (text, is_error) = c.tool("build_overview", json!({}));
    assert!(is_error && text.contains("No build yet"), "{text}");
    let (text, is_error) = c.tool("compare_with_guide", json!({ "guide": GUIDE }));
    assert!(is_error && text.contains("No build yet"), "{text}");
    let (text, is_error) = c.tool("load_build", json!({ "build": GUIDE }));
    assert!(!is_error, "{text}");
    let (text, is_error) = c.tool("build_overview", json!({}));
    assert!(!is_error, "{text}");
    let overview: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(overview["main_skill"], "Caustic Arrow");
}
