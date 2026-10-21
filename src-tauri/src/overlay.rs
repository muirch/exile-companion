use std::sync::atomic::Ordering;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

const OVERLAY_LABEL: &str = "overlay";

/// Grabbed means the overlay takes the mouse (drag, buttons); otherwise clicks go to the game.
pub fn set_grab(app: &AppHandle, grabbed: bool) {
    let state = app.state::<AppState>();
    state.grabbed.store(grabbed, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.set_ignore_cursor_events(!grabbed);
    }
    let _ = app.emit("overlay://grab-changed", grabbed);
}

pub fn toggle_grab(app: &AppHandle) {
    let grabbed = app.state::<AppState>().grabbed.load(Ordering::SeqCst);
    set_grab(app, !grabbed);
}

pub fn game_focused(app: &AppHandle) -> bool {
    app.state::<AppState>().game_focused.load(Ordering::SeqCst)
}

// The overlay has no taskbar button, so the tray is the only way to reach it outside the game.
pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let grab = MenuItem::with_id(
        app,
        "grab",
        "Move / settings (Ctrl+Alt+Space)",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit poebo", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&grab, &quit])?;
    let mut tray = TrayIconBuilder::new()
        .tooltip("poebo")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "grab" => set_grab(app, true),
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Hides the overlay while neither the game nor the overlay itself is the foreground window.
/// The window stays shown and the UI fades out: showing a hidden window would steal focus from
/// the game. Only implemented on Windows; elsewhere the overlay is always visible.
pub fn watch_game_focus(app: AppHandle) {
    #[cfg(windows)]
    std::thread::spawn(move || {
        let mut last = None;
        loop {
            let own = app
                .get_webview_window(OVERLAY_LABEL)
                .and_then(|w| w.hwnd().ok())
                .map(|h| h.0);
            let focused = win::foreground_is_game_or(own);
            if last != Some(focused) {
                last = Some(focused);
                app.state::<AppState>()
                    .game_focused
                    .store(focused, Ordering::SeqCst);
                if !focused && app.state::<AppState>().grabbed.load(Ordering::SeqCst) {
                    set_grab(&app, false);
                }
                let _ = app.emit("overlay://game-focus", focused);
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    });
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowTextW,
    };

    // PoE 1 and PoE 2 both use this window class; the title check covers anything else.
    const GAME_CLASS: &str = "POEWindowClass";
    const GAME_TITLE: &str = "Path of Exile";

    pub fn foreground_is_game_or(own: Option<*mut c_void>) -> bool {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_null() {
            return false;
        }
        if own == Some(hwnd) {
            return true;
        }
        let mut buf = [0u16; 256];
        let len = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        if String::from_utf16_lossy(&buf[..len.max(0) as usize]) == GAME_CLASS {
            return true;
        }
        let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        // Exact match: a browser tab titled "Path of Exile Wiki" must not count.
        let title = String::from_utf16_lossy(&buf[..len.max(0) as usize]);
        title == GAME_TITLE || title == "Path of Exile 2"
    }
}
