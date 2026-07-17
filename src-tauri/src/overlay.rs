//! Overlay window behavior: default click-through + the grab/release toggle.

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

const OVERLAY_LABEL: &str = "overlay";

/// Make the overlay click-through by default so the game receives the mouse.
pub fn set_click_through_default(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.set_ignore_cursor_events(true);
    }
}

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
