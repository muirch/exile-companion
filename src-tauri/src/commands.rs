//! The Tauri command surface. These are thin; the work lives in `runtime`/`settings`/`overlay`.
//! Errors cross the IPC boundary as `String`; core keeps its typed errors.

use std::path::Path;
use std::time::Duration;

use poebo_core::build::BuildSummary;
use poebo_core::guidance::NextObjective;
use poebo_core::route::RouteView;
use poebo_core::source::SpeedMultiplier;
use poebo_core::state::CampaignState;
use tauri::{AppHandle, Emitter, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::runtime::{self, RouteMove};
use crate::settings::{self, Settings};
use crate::state::AppState;

/// Open a file dialog to pick a `Client.txt` (or recording); returns the chosen path.
#[tauri::command]
pub async fn pick_log_file(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    // Owned by the calling window, which stays on top: a dialog without an owner opens under it.
    app.dialog()
        .file()
        .set_parent(&window)
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
pub async fn import_build_file(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<Option<BuildSummary>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(&window)
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
    let text = std::fs::read(Path::new(&path)).map_err(|e| e.to_string())?;
    let xml =
        poebo_pob::xml_from_text(&String::from_utf8_lossy(&text)).map_err(|e| e.to_string())?;
    runtime::import_build(&app, xml).map(Some)
}

/// Import a Path of Building build from pasted text: a build link, an import code or raw XML.
#[tauri::command]
pub async fn import_build_code(app: AppHandle, code: String) -> Result<BuildSummary, String> {
    let code = match poebo_pob::code_url(&code) {
        Some(url) => tauri::async_runtime::spawn_blocking(move || fetch_build_code(&url))
            .await
            .map_err(|e| e.to_string())??,
        None => code,
    };
    let xml = poebo_pob::xml_from_text(&code).map_err(|e| {
        log::warn!("build import: {e}");
        "That's not a Path of Building code or a build link.".to_string()
    })?;
    runtime::import_build(&app, xml)
}

fn fetch_build_code(url: &str) -> Result<String, String> {
    // The default roots are a bundled Mozilla list that schannel rejects; use the Windows store.
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(15)))
        .user_agent(concat!("poebo/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    log::info!("downloading build from {url}");
    agent
        .get(url)
        .call()
        .and_then(|mut r| r.body_mut().read_to_string())
        .map_err(|e| format!("Couldn't download the build: {e}"))
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayState {
    grabbed: bool,
    game_focused: bool,
}

#[tauri::command]
pub fn get_overlay_state(app: AppHandle, state: State<'_, AppState>) -> OverlayState {
    OverlayState {
        grabbed: state.grabbed.load(std::sync::atomic::Ordering::SeqCst),
        game_focused: crate::overlay::game_focused(&app),
    }
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    crate::menu::open_settings(&app);
}

#[tauri::command]
pub fn start_move(app: AppHandle) {
    crate::menu::start_move(&app);
}

#[tauri::command]
pub fn lock_overlay(app: AppHandle) {
    crate::overlay::set_grab(&app, false);
}

#[tauri::command]
pub fn close_menu(app: AppHandle) {
    crate::menu::close(&app);
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn get_route_enabled(app: AppHandle) -> bool {
    crate::menu::route_enabled(&app)
}

#[tauri::command]
pub fn set_route_enabled(app: AppHandle, enabled: bool) {
    crate::menu::set_route_enabled(&app, enabled);
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeySettings {
    hotkeys: crate::hotkeys::Hotkeys,
    relog_enabled: bool,
}

#[tauri::command]
pub fn get_hotkeys(app: AppHandle) -> HotkeySettings {
    let s = settings::load(&app).unwrap_or_default();
    HotkeySettings {
        hotkeys: s.hotkeys,
        relog_enabled: s.relog_enabled,
    }
}

/// Saves and re-registers; returns the bindings that failed.
#[tauri::command]
pub fn set_hotkeys(
    app: AppHandle,
    value: HotkeySettings,
) -> Result<Vec<crate::hotkeys::BindError>, String> {
    let mut s = settings::load(&app)?;
    s.hotkeys = value.hotkeys;
    s.relog_enabled = value.relog_enabled;
    settings::save(&app, &s)?;
    let _ = app.emit(
        "hotkeys://changed",
        HotkeySettings {
            hotkeys: s.hotkeys.clone(),
            relog_enabled: s.relog_enabled,
        },
    );
    Ok(crate::hotkeys::register_all(&app))
}

// While the settings window records a new shortcut, the old ones must not fire.
#[tauri::command]
pub fn pause_hotkeys(app: AppHandle) {
    crate::hotkeys::pause(&app);
}

#[tauri::command]
pub fn resume_hotkeys(app: AppHandle) -> Vec<crate::hotkeys::BindError> {
    crate::hotkeys::register_all(&app)
}

#[tauri::command]
pub fn fit_overlay(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    crate::overlay::fit(&app, width, height).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn snap_overlay_bottom_center(app: AppHandle) -> Result<(), String> {
    crate::overlay::snap_bottom_center(&app).map_err(|e| e.to_string())
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
    settings::save(&app, &settings)?;
    // The overlay and settings windows each hold their own copy of the display settings.
    let _ = app.emit("settings://changed", &settings);
    crate::runtime::emit_route(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    // Hits pobb.in, so it only runs where the Windows CI job asks for it.
    #[test]
    #[ignore]
    fn downloads_a_build_over_tls() {
        let url = poebo_pob::code_url("https://pobb.in/NlUa8ZnB3YGK").unwrap();
        let code = super::fetch_build_code(&url).unwrap();
        let build = poebo_pob::import_text(&code).unwrap();
        assert_eq!(build.class_name.as_deref(), Some("Ranger"));
    }
}
