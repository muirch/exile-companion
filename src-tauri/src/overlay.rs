//! Overlay window behavior: the interactive/click-through toggle.
//!
//! The overlay starts interactive (its controls are usable directly). On X11 the global hotkey
//! flips it to click-through so the game receives the mouse; on Wayland global shortcuts do not
//! fire, so it simply stays interactive.

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

const OVERLAY_LABEL: &str = "overlay";

/// Toggle interactive ("grabbed") mode: when grabbed the overlay honors the cursor (so it can be
/// dragged and its controls clicked); otherwise clicks pass through to the game. Emits
/// `overlay://grab-changed` so the UI can reflect the state.
pub fn toggle_grab(app: &AppHandle) {
    let state = app.state::<AppState>();
    let grabbed = !state.grabbed.fetch_xor(true, Ordering::SeqCst);

    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        // grabbed => interactive => do NOT ignore cursor events.
        let _ = window.set_ignore_cursor_events(!grabbed);
    }
    let _ = app.emit("overlay://grab-changed", grabbed);
}
