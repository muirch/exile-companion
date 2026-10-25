//! The Tauri command surface. These are thin. The work lives in `runtime`/`settings`/`overlay`.
//! Errors cross the IPC boundary as `String`. Core keeps its typed errors.

use std::path::Path;
use std::time::Duration;

use exile_core::build::BuildSummary;
use exile_core::guidance::NextObjective;
use exile_core::route::RouteView;
use exile_core::source::SpeedMultiplier;
use exile_core::state::CampaignState;
use tauri::{AppHandle, Emitter, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::runtime::{self, RouteMove};
use crate::settings::{self, Settings};
use crate::state::AppState;

/// Open a file dialog to pick a `Client.txt` (or recording). Returns the chosen path.
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

/// Path of Building's folder: the saved one, or the first found in the registry and the usual places.
#[tauri::command]
pub fn get_pob(app: AppHandle) -> Option<crate::pob::PobInfo> {
    crate::pob::find(&app)
}

#[tauri::command]
pub async fn pick_pob_folder(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<Option<crate::pob::PobInfo>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title("Path of Building folder")
        .pick_folder(move |dir| {
            let _ = tx.send(dir);
        });
    let Some(dir) = rx.await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let dir = dir.into_path().map_err(|e| e.to_string())?;
    crate::pob::choose(&app, &dir).map(Some)
}

#[tauri::command]
pub fn get_pob_sync(app: AppHandle) -> crate::pob::PobSync {
    crate::pob::sync_view(&app)
}

/// Syncs right away, so the file shows up in PoB's list before the player looks.
#[tauri::command]
pub fn set_pob_sync(app: AppHandle, enabled: bool) -> Result<crate::pob::PobSync, String> {
    let mut s = settings::load(&app)?;
    s.pob_sync = enabled;
    settings::save(&app, &s)?;
    runtime::sync_pob(&app, &mut None);
    Ok(crate::pob::sync_view(&app))
}

/// The build file the player imports their character into in PoB. It replaces the app's build
/// now, and the two stay the same from then on.
#[tauri::command]
pub async fn pick_pob_build(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<Option<crate::pob::PobSync>, String> {
    let s = settings::load(&app)?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut dialog = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Build in Path of Building")
        .add_filter("Path of Building build", &["xml"]);
    if let Some(dir) = crate::pob::builds_dir(&app, &s) {
        dialog = dialog.set_directory(dir);
    }
    dialog.pick_file(move |file| {
        let _ = tx.send(file);
    });
    let Some(picked) = rx.await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let xml = std::fs::read(&path).map_err(|e| e.to_string())?;
    runtime::import_build(&app, xml)?;
    let mut s = settings::load(&app)?;
    s.pob_sync = true;
    s.pob_build = Some(path.to_string_lossy().into_owned());
    settings::save(&app, &s)?;
    Ok(Some(crate::pob::sync_view(&app)))
}

/// Back to Exile Companion's own file in PoB's Builds folder.
#[tauri::command]
pub fn unpick_pob_build(app: AppHandle) -> Result<crate::pob::PobSync, String> {
    let mut s = settings::load(&app)?;
    s.pob_build = None;
    settings::save(&app, &s)?;
    runtime::sync_pob(&app, &mut None);
    Ok(crate::pob::sync_view(&app))
}

#[tauri::command]
pub fn open_pob(app: AppHandle) -> Result<(), String> {
    crate::pob::open(&app)
}

#[tauri::command]
pub fn get_assistant() -> crate::assistant::AssistantView {
    crate::assistant::view()
}

#[tauri::command]
pub fn connect_agent(
    id: exile_core::agents::AgentId,
) -> Result<crate::assistant::AssistantView, String> {
    crate::assistant::connect(id)
}

/// Puts the generic MCP config on the clipboard, for agents the app doesn't know.
#[tauri::command]
pub fn copy_assistant_config(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let snippet = crate::assistant::snippet()
        .ok_or("The assistant isn't installed with this build of Exile Companion.")?;
    app.clipboard()
        .write_text(snippet)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_chat(app: AppHandle) -> crate::chat::ChatView {
    crate::chat::view(&app).await
}

#[tauri::command]
pub async fn set_api_key(
    app: AppHandle,
    provider: exile_assistant::Provider,
    key: String,
) -> Result<crate::chat::ChatView, String> {
    crate::chat::set_key(&app, provider, key).await
}

#[tauri::command]
pub async fn forget_api_key(
    app: AppHandle,
    provider: exile_assistant::Provider,
) -> Result<crate::chat::ChatView, String> {
    crate::chat::forget_key(&app, provider).await
}

#[tauri::command]
pub async fn get_next_card(app: AppHandle) -> exile_assistant::NextCard {
    crate::chat::next_card(&app).await
}

/// Starts an answer. Its text arrives as chat://event, the end as chat://over.
#[tauri::command]
pub fn chat_send(app: AppHandle, question: String) -> Result<(), String> {
    crate::chat::send(&app, question)
}

#[tauri::command]
pub fn chat_stop(app: AppHandle) {
    crate::chat::stop(&app);
}

#[tauri::command]
pub async fn chat_new(app: AppHandle) {
    crate::chat::new_chat(&app).await;
}

#[tauri::command]
pub fn open_chat(app: AppHandle) {
    crate::chat::open(&app);
}

#[tauri::command]
pub fn close_chat(app: AppHandle) {
    crate::chat::close(&app);
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
        exile_pob::xml_from_text(&String::from_utf8_lossy(&text)).map_err(|e| e.to_string())?;
    runtime::import_build(&app, xml).map(Some)
}

/// Import a Path of Building build from pasted text: a build link, an import code or raw XML.
#[tauri::command]
pub async fn import_build_code(app: AppHandle, code: String) -> Result<BuildSummary, String> {
    let code = match exile_pob::code_url(&code) {
        Some(url) => tauri::async_runtime::spawn_blocking(move || fetch_build_code(&url))
            .await
            .map_err(|e| e.to_string())??,
        None => code,
    };
    let xml = exile_pob::xml_from_text(&code).map_err(|e| {
        log::warn!("build import: {e}");
        "That's not a Path of Building code or a build link.".to_string()
    })?;
    runtime::import_build(&app, xml)
}

fn fetch_build_code(url: &str) -> Result<String, String> {
    // The default roots are a bundled Mozilla list that schannel rejects. Use the Windows store.
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(15)))
        .user_agent(concat!("exile-companion/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    log::info!("downloading build from {url}");
    agent
        .get(url)
        .call()
        .and_then(|mut r| r.body_mut().read_to_string())
        .map_err(|e| {
            log::warn!("build download from {url}: {e}");
            "Couldn't download the build. Check the link and your connection.".to_string()
        })
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
pub fn get_gem_rows(app: AppHandle) -> Vec<runtime::GemRow> {
    runtime::gem_rows(&app)
}

#[tauri::command]
pub fn set_gem_override(
    app: AppHandle,
    name: String,
    over: Option<exile_core::gemplan::GemOverride>,
) -> Result<(), String> {
    runtime::set_gem_override(&app, name, over)
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
    hideout_enabled: bool,
    item_check_enabled: bool,
    vendor_search_enabled: bool,
}

#[tauri::command]
pub fn get_hotkeys(app: AppHandle) -> HotkeySettings {
    let s = settings::load(&app).unwrap_or_default();
    HotkeySettings {
        hotkeys: s.hotkeys,
        relog_enabled: s.relog_enabled,
        hideout_enabled: s.hideout_enabled,
        item_check_enabled: s.item_check_enabled,
        vendor_search_enabled: s.vendor_search_enabled,
    }
}

/// Saves and re-registers. Returns the bindings that failed.
#[tauri::command]
pub fn set_hotkeys(
    app: AppHandle,
    value: HotkeySettings,
) -> Result<Vec<crate::hotkeys::BindError>, String> {
    let mut s = settings::load(&app)?;
    s.hotkeys = value.hotkeys;
    s.relog_enabled = value.relog_enabled;
    s.hideout_enabled = value.hideout_enabled;
    s.item_check_enabled = value.item_check_enabled;
    s.vendor_search_enabled = value.vendor_search_enabled;
    settings::save(&app, &s)?;
    let _ = app.emit(
        "hotkeys://changed",
        HotkeySettings {
            hotkeys: s.hotkeys.clone(),
            relog_enabled: s.relog_enabled,
            hideout_enabled: s.hideout_enabled,
            item_check_enabled: s.item_check_enabled,
            vendor_search_enabled: s.vendor_search_enabled,
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
    // Preset stages decide which gems the campaign can still give.
    crate::runtime::emit_build_summary(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    // Hits pobb.in, so it only runs where the Windows CI job asks for it.
    #[test]
    #[ignore]
    fn downloads_a_build_over_tls() {
        let url = exile_pob::code_url("https://pobb.in/NlUa8ZnB3YGK").unwrap();
        let code = super::fetch_build_code(&url).unwrap();
        let build = exile_pob::import_text(&code).unwrap();
        assert_eq!(build.class_name.as_deref(), Some("Ranger"));
    }
}
