//! Persisted user settings, stored as JSON in the app config directory.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// User settings. Field names are camelCase to match the frontend `Settings` type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub opacity: f64,
    pub last_log_path: Option<String>,
    pub route_edge: usize,
    pub text_scale: f64,
    /// Overlay position as fractions of its monitor: horizontal center and bottom edge.
    pub anchor: Option<(f64, f64)>,
    pub route_enabled: bool,
    pub hotkeys: crate::hotkeys::Hotkeys,
    /// Off by default: it is the one feature that sends input to the game.
    pub relog_enabled: bool,
    /// Read once at startup: the edges differ between the two route variants.
    pub league_start: bool,
    pub gem_hints: poebo_core::gemplan::GemHints,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            opacity: 0.8,
            last_log_path: None,
            route_edge: 0,
            text_scale: 1.0,
            anchor: None,
            route_enabled: true,
            hotkeys: crate::hotkeys::Hotkeys::default(),
            relog_enabled: false,
            league_start: true,
            gem_hints: Default::default(),
        }
    }
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Result<Settings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut settings: Settings = serde_json::from_str(&data).unwrap_or_default();
    settings.hotkeys.normalize();
    Ok(settings)
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app)?;
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}
