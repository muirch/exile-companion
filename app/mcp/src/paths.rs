//! Where the desktop app keeps its files. Tauri's app_config_dir and app_cache_dir for the
//! identifier com.exilecompanion.app, worked out the same way here because this binary has no
//! Tauri.

use std::path::PathBuf;

const IDENTIFIER: &str = "com.exilecompanion.app";

fn env(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn home() -> Option<PathBuf> {
    env("HOME").or_else(|| env("USERPROFILE"))
}

pub fn config_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        env("APPDATA")
    } else if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library/Application Support"))
    } else {
        env("XDG_CONFIG_HOME").or_else(|| home().map(|h| h.join(".config")))
    };
    base.map(|b| b.join(IDENTIFIER))
}

pub fn cache_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        env("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library/Caches"))
    } else {
        env("XDG_CACHE_HOME").or_else(|| home().map(|h| h.join(".cache")))
    };
    base.map(|b| b.join(IDENTIFIER))
}

/// Where the app keeps the player's build, as PoB XML, whether or not there is one yet.
pub fn build_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join("build.xml"))
}

fn settings() -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(config_dir()?.join("settings.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// The PoB folder the app found or the player picked, from settings.json.
pub fn saved_pob() -> Option<PathBuf> {
    settings()?.get("pobPath")?.as_str().map(PathBuf::from)
}

/// The Client.txt the app follows, from settings.json.
pub fn saved_log() -> Option<String> {
    settings()?.get("lastLogPath")?.as_str().map(str::to_string)
}

/// The engine ships next to this binary.
pub fn engine_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let engine = exe
        .parent()?
        .join(format!("exile-pob-engine{}", std::env::consts::EXE_SUFFIX));
    engine.is_file().then_some(engine)
}
