//! The Tauri command surface. These are thin; the work lives in `runtime`/`settings`/`overlay`.
//! Errors cross the IPC boundary as `String`; core keeps its typed errors.

use std::path::Path;

use poebo_core::build::BuildSummary;
use poebo_core::guidance::NextObjective;
use poebo_core::route::RouteView;
use poebo_core::source::SpeedMultiplier;
use poebo_core::state::CampaignState;
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::runtime::{self, RouteMove};
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

/// Open a file dialog to pick a Path of Building export (`.xml` or a saved code), import it, and
/// return its summary. `None` if the dialog was cancelled.
#[tauri::command]
pub async fn import_build_file(app: AppHandle) -> Result<Option<BuildSummary>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Path of Building", &["xml", "txt"])
        .pick_file(move |file| {
            let _ = tx.send(file);
        });
    let Some(picked) = rx.await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let path = picked
        .into_path()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .into_owned();
    let build = poebo_pob::from_local_file(Path::new(&path)).map_err(|e| e.to_string())?;
    runtime::import_build(&app, build).map(Some)
}

/// Import a Path of Building build from pasted text (an import code or raw XML).
#[tauri::command]
pub fn import_build_code(app: AppHandle, code: String) -> Result<BuildSummary, String> {
    let build = poebo_pob::import_text(&code).map_err(|e| e.to_string())?;
    runtime::import_build(&app, build)
}

/// Forget the currently-loaded build.
#[tauri::command]
pub fn clear_build(app: AppHandle) {
    runtime::clear_build(&app);
}

/// The current build summary, or `None` if no build is loaded.
#[tauri::command]
pub fn get_build(app: AppHandle) -> Option<BuildSummary> {
    runtime::current_build_summary(&app)
}

/// The next objective for the current state + loaded build (on-demand hydration).
#[tauri::command]
pub fn get_guidance(app: AppHandle) -> NextObjective {
    runtime::current_guidance(&app)
}

#[tauri::command]
pub fn get_source(app: AppHandle) -> runtime::SourceStatus {
    runtime::current_source(&app)
}

#[tauri::command]
pub fn get_route(app: AppHandle) -> Option<RouteView> {
    runtime::current_route_view(&app)
}

#[tauri::command]
pub fn route_next(app: AppHandle) -> Option<RouteView> {
    runtime::move_route(&app, RouteMove::Next)
}

#[tauri::command]
pub fn route_prev(app: AppHandle) -> Option<RouteView> {
    runtime::move_route(&app, RouteMove::Prev)
}

#[tauri::command]
pub fn route_reset(app: AppHandle) -> Option<RouteView> {
    runtime::move_route(&app, RouteMove::Reset)
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
