//! The gems window: the build's links for the character's stage, shown with the quick menu while
//! the menu key is held, like a game overlay layer. It goes when neither of them has the focus.

use std::sync::OnceLock;
use std::time::Duration;

use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition};

use crate::settings;
use crate::state::AppState;

pub const GEMS_LABEL: &str = "gems";
// The window's sizes in tauri.conf.json, at 1080p.
const SIZE: (f64, f64) = (340.0, 540.0);
const MIN_SIZE: (f64, f64) = (300.0, 320.0);

static SIZED: OnceLock<()> = OnceLock::new();

/// Scaled to the monitor like the overlay the first time, then the size the player gave it, on
/// the left unless the player moved it.
fn place(app: &AppHandle, window: &tauri::WebviewWindow) {
    let Some(m) = app
        .get_webview_window("overlay")
        .and_then(|w| w.current_monitor().ok().flatten())
    else {
        return;
    };
    let (mp, ms) = (m.position(), m.size());
    let zoom = crate::chat::screen_zoom(ms.height as f64 / m.scale_factor());
    let scaled = |(w, h): (f64, f64)| LogicalSize::new(w * zoom, h * zoom);
    let (width, height) = if SIZED.set(()).is_ok() {
        let _ = window.set_zoom(zoom);
        let _ = window.set_min_size(Some(scaled(MIN_SIZE)));
        let _ = window.set_size(scaled(SIZE));
        let px = |v: f64| (v * zoom * m.scale_factor()) as i32;
        (px(SIZE.0), px(SIZE.1))
    } else {
        let size = window.outer_size().unwrap_or_default();
        (size.width as i32, size.height as i32)
    };
    let max_x = (ms.width as i32 - width).max(0);
    let max_y = (ms.height as i32 - height).max(0);
    let (x, y) = match settings::load(app).ok().and_then(|s| s.gems_anchor) {
        Some((fx, fy)) => (
            (fx * ms.width as f64) as i32,
            (fy * ms.height as f64) as i32,
        ),
        None => (ms.width as i32 / 40, ms.height as i32 / 8),
    };
    let _ = window.set_position(PhysicalPosition::new(
        mp.x + x.clamp(0, max_x),
        mp.y + y.clamp(0, max_y),
    ));
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
    let (Ok(pos), Ok(Some(m))) = (window.outer_position(), window.current_monitor()) else {
        return;
    };
    let (mp, ms) = (m.position(), m.size());
    let fx = (pos.x - mp.x) as f64 / ms.width as f64;
    let fy = (pos.y - mp.y) as f64 / ms.height as f64;
    if let Ok(mut s) = settings::load(app) {
        s.gems_anchor = Some((fx.clamp(0.0, 1.0), fy.clamp(0.0, 1.0)));
        let _ = settings::save(app, &s);
    }
}

/// Focus moves from the menu to the gems window when the player clicks it. Only when neither has
/// it a moment later did the player go back to the game.
pub fn close_layer_if_unfocused(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let focused = |label: &str| {
            app.get_webview_window(label)
                .and_then(|w| w.is_focused().ok())
                .unwrap_or(false)
        };
        if !focused(GEMS_LABEL) && !focused(crate::menu::MENU_LABEL) {
            let app2 = app.clone();
            let _ = app.run_on_main_thread(move || {
                hide(&app2);
                crate::menu::close(&app2);
            });
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
