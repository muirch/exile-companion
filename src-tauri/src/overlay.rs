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

// PoE's skill bar and flasks take roughly the bottom 15% of the screen.
const ABOVE_SKILL_BAR: f64 = 0.15;

pub fn snap_bottom_center(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(OVERLAY_LABEL) else {
        return Ok(());
    };
    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };
    let (mpos, msize) = (monitor.position(), monitor.size());
    let size = window.outer_size()?;
    let x = mpos.x + (msize.width as i32 - size.width as i32) / 2;
    let bottom = mpos.y + msize.height as i32 - (msize.height as f64 * ABOVE_SKILL_BAR) as i32;
    window.set_position(tauri::PhysicalPosition::new(x, bottom - size.height as i32))
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
            "grab" => {
                set_grab(app, true);
                // Without focus the next foreground change (the menu closing) would lock it again.
                if let Some(w) = app.get_webview_window(OVERLAY_LABEL) {
                    let _ = w.set_focus();
                }
            }
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
/// The window stays shown and the UI goes transparent: showing a hidden window would steal focus
/// from the game. Only implemented on Windows; elsewhere the overlay is always visible.
pub fn watch_game_focus(app: AppHandle) {
    #[cfg(windows)]
    win::watch(app);
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(windows)]
fn on_foreground_changed(app: &AppHandle, is_game: bool, is_own: bool) {
    let state = app.state::<AppState>();
    // Alt-tabbing away while unlocked locks again, so the overlay never eats desktop clicks.
    if !is_game && !is_own && state.grabbed.load(Ordering::SeqCst) {
        set_grab(app, false);
    }
    let focused = is_game || is_own;
    if state.game_focused.swap(focused, Ordering::SeqCst) != focused {
        let _ = app.emit("overlay://game-focus", focused);
    }
}

#[cfg(windows)]
mod win {
    use std::sync::OnceLock;

    use tauri::{AppHandle, Manager};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetClassNameW, GetForegroundWindow, GetMessageW, GetWindowTextW,
        TranslateMessage, EVENT_SYSTEM_FOREGROUND, MSG, WINEVENT_OUTOFCONTEXT,
    };

    use super::OVERLAY_LABEL;

    // PoE 1 and PoE 2 both use this window class; the title check covers anything else.
    const GAME_CLASS: &str = "POEWindowClass";
    const GAME_TITLES: [&str; 2] = ["Path of Exile", "Path of Exile 2"];

    // The hook callback has no user data pointer.
    static APP: OnceLock<AppHandle> = OnceLock::new();

    pub fn watch(app: AppHandle) {
        if APP.set(app).is_err() {
            return;
        }
        std::thread::spawn(|| unsafe {
            report(GetForegroundWindow());
            // Out-of-context hooks are delivered through this thread's message loop.
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                std::ptr::null_mut(),
                Some(on_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
    }

    unsafe extern "system" fn on_event(
        _hook: HWINEVENTHOOK,
        _event: u32,
        hwnd: HWND,
        _object: i32,
        _child: i32,
        _thread: u32,
        _time: u32,
    ) {
        report(hwnd);
    }

    fn report(hwnd: HWND) {
        let Some(app) = APP.get() else {
            return;
        };
        let own = app
            .get_webview_window(OVERLAY_LABEL)
            .and_then(|w| w.hwnd().ok())
            .map(|h| h.0);
        let is_own = !hwnd.is_null() && own == Some(hwnd);
        super::on_foreground_changed(app, !is_own && is_game(hwnd), is_own);
    }

    fn is_game(hwnd: HWND) -> bool {
        if hwnd.is_null() {
            return false;
        }
        let mut buf = [0u16; 256];
        let len = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        if String::from_utf16_lossy(&buf[..len.max(0) as usize]) == GAME_CLASS {
            return true;
        }
        // Exact match: a browser tab titled "Path of Exile Wiki" must not count.
        let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        let title = String::from_utf16_lossy(&buf[..len.max(0) as usize]);
        GAME_TITLES.contains(&title.as_str())
    }
}
