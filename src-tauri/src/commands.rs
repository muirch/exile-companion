//! The Tauri command surface. These are thin; the work lives in `runtime`/`settings`/`overlay`.
//! Errors cross the IPC boundary as `String`; core keeps its typed errors.

use poebo_core::source::SpeedMultiplier;
use poebo_core::state::CampaignState;
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::runtime;
use crate::settings::{self, Settings};
use crate::state::AppState;

/// Open a file dialog to pick a `Client.txt` (or recording); returns the chosen path.
#[tauri::command]
pub async fn pick_log_file(app: AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("PoE log", &["txt", "log"])
        .pick_file(move |file| {
            let _ = tx.send(file);
        });
    let picked = rx.await.map_err(|e| e.to_string())?;
    Ok(picked
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
pub fn start_live(app: AppHandle, path: String) -> Result<CampaignState, String> {
    runtime::start_live(app, path)
}

#[tauri::command]
pub fn stop_source(app: AppHandle) -> Result<(), String> {
    runtime::stop_current(&app);
    Ok(())
}

#[tauri::command]
pub fn start_replay(app: AppHandle, path: String, speed: SpeedMultiplier) -> Result<(), String> {
    runtime::start_replay(app, path, speed)
}

#[tauri::command]
pub fn replay_pause(app: AppHandle) {
    runtime::replay_pause(&app);
}

#[tauri::command]
pub fn replay_resume(app: AppHandle) {
    runtime::replay_resume(&app);
}

#[tauri::command]
pub fn replay_step(app: AppHandle) {
    runtime::replay_step(&app);
}

#[tauri::command]
pub fn replay_set_speed(app: AppHandle, speed: SpeedMultiplier) {
    runtime::replay_set_speed(&app, speed);
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> CampaignState {
    state.current_state()
}

#[tauri::command]
pub fn set_click_through(window: WebviewWindow, ignore: bool) -> Result<(), String> {
    window
        .set_ignore_cursor_events(ignore)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<Settings, String> {
    settings::load(&app)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    settings::save(&app, &settings)
}
