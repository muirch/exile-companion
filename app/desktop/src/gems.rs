//! The gems window: the build's links for the character's stage, shown with the quick menu while
//! the menu key is held, like a game overlay layer. It goes when no window of the layer has the
//! focus.

use std::sync::OnceLock;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::settings;
use crate::state::AppState;

pub const GEMS_LABEL: &str = "gems";
// The window's sizes in tauri.conf.json, at 1080p.
const SIZE: (f64, f64) = (340.0, 540.0);
const MIN_SIZE: (f64, f64) = (300.0, 320.0);

static SIZED: OnceLock<()> = OnceLock::new();

fn place(app: &AppHandle, window: &tauri::WebviewWindow) {
    let anchor = settings::load(app).ok().and_then(|s| s.gems_anchor);
    crate::panel::place(
        app,
        window,
        &SIZED,
        crate::panel::Place {
            size: SIZE,
            min_size: MIN_SIZE,
            anchor,
            default_anchor: (1.0 / 40.0, 1.0 / 8.0),
        },
    );
}

/// Shows the window without taking the focus from the menu.
pub fn show(app: &AppHandle) {
    let Some(w) = app.get_webview_window(GEMS_LABEL) else {
        return;
    };
    if crate::runtime::plan_source(app).is_none() {
        return;
    }
    place(app, &w);
    let _ = w.show();
    let _ = app.emit_to(GEMS_LABEL, "gems://opened", ());
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(GEMS_LABEL) {
        if w.is_visible().unwrap_or(false) {
            remember_place(app, &w);
            let _ = w.hide();
        }
    }
}

fn remember_place(app: &AppHandle, window: &tauri::WebviewWindow) {
    let Some(spot) = crate::panel::spot(window) else {
        return;
    };
    if let Ok(mut s) = settings::load(app) {
        s.gems_anchor = Some(spot.anchor);
        let _ = settings::save(app, &s);
    }
}

/// Focus moves from the menu to the gems or tree window when the player clicks it. Only when none
/// of them has it a moment later did the player go back to the game.
pub fn close_layer_if_unfocused(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let focused = |label: &str| {
            app.get_webview_window(label)
                .and_then(|w| w.is_focused().ok())
                .unwrap_or(false)
        };
        let layer = [GEMS_LABEL, crate::tree::TREE_LABEL, crate::menu::MENU_LABEL];
        if !layer.into_iter().any(focused) {
            let app2 = app.clone();
            let _ = app.run_on_main_thread(move || crate::menu::close(&app2));
        }
    });
}

/// The notes the author wrote in the PoB the route's gems come from.
pub fn notes(app: &AppHandle) -> Option<String> {
    crate::runtime::plan_source(app)?.notes
}

/// The links of skill set `set`, or of the one the character's level is at.
pub fn links(app: &AppHandle, set: Option<usize>) -> Option<exile_core::links::Links> {
    let build = crate::runtime::plan_source(app)?;
    let state = app.state::<AppState>();
    let set = set
        .unwrap_or_else(|| exile_core::links::set_for_level(&build, state.current_state().level));
    let plan = crate::runtime::current_gem_plan(app);
    Some(exile_core::links::links(
        &build,
        set,
        &state.gems,
        plan.as_ref(),
    ))
}
