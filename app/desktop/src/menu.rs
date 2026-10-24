use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager};

use crate::overlay;
use crate::settings;
use crate::state::AppState;

pub const MENU_LABEL: &str = "menu";
pub const SETTINGS_LABEL: &str = "settings";

/// Opens the quick menu with its top-left corner at the cursor, kept on the cursor's monitor.
#[cfg(windows)]
pub fn open_at_cursor(app: &AppHandle) {
    let Some(menu) = app.get_webview_window(MENU_LABEL) else {
        return;
    };
    if let (Ok(cursor), Ok(size)) = (menu.cursor_position(), menu.outer_size()) {
        let (cx, cy) = (cursor.x as i32, cursor.y as i32);
        let mut x = cx;
        let mut y = cy;
        if let Ok(Some(m)) = menu.monitor_from_point(cursor.x, cursor.y) {
            let (mp, ms) = (m.position(), m.size());
            x = x.min(mp.x + ms.width as i32 - size.width as i32);
            y = y.min(mp.y + ms.height as i32 - size.height as i32);
        }
        let _ = menu.set_position(tauri::PhysicalPosition::new(x, y));
    }
    let _ = app.emit("menu://opened", ());
    let _ = menu.show();
    let _ = menu.set_focus();
}

pub fn close(app: &AppHandle) {
    if let Some(menu) = app.get_webview_window(MENU_LABEL) {
        let _ = menu.hide();
    }
}

pub fn open_settings(app: &AppHandle) {
    close(app);
    if let Some(w) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn start_move(app: &AppHandle) {
    close(app);
    overlay::set_grab(app, true);
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.set_focus();
    }
}

pub fn route_enabled(app: &AppHandle) -> bool {
    app.state::<AppState>().route_enabled.load(Ordering::SeqCst)
}

pub fn set_route_enabled(app: &AppHandle, enabled: bool) {
    app.state::<AppState>()
        .route_enabled
        .store(enabled, Ordering::SeqCst);
    if let Ok(mut s) = settings::load(app) {
        s.route_enabled = enabled;
        let _ = settings::save(app, &s);
    }
    let _ = app.emit("features://route", enabled);
}

/// Which key is watched for the hold: "Backquote" or "Tab".
pub fn set_hold_key(name: &str) {
    #[cfg(windows)]
    win::set_hold_key(name);
    #[cfg(not(windows))]
    let _ = name;
}

/// Holding the menu key opens the menu. The hook only watches: every key event is passed on to
/// the game untouched.
pub fn watch_menu_hold(app: AppHandle) {
    #[cfg(windows)]
    win::watch(app);
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(windows)]
mod win {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;

    use exile_core::hold::HoldKey;
    use tauri::AppHandle;
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;

    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_OEM_3, VK_TAB};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
        KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    const HOLD: Duration = Duration::from_millis(450);

    static APP: OnceLock<AppHandle> = OnceLock::new();
    static HOLD_KEY: Mutex<HoldKey> = Mutex::new(HoldKey::new());
    static HOLD_VK: AtomicU32 = AtomicU32::new(VK_OEM_3 as u32);

    pub fn set_hold_key(name: &str) {
        let vk = match name {
            "Tab" => VK_TAB,
            _ => VK_OEM_3,
        };
        HOLD_VK.store(u32::from(vk), Ordering::SeqCst);
    }

    pub fn watch(app: AppHandle) {
        if APP.set(app).is_err() {
            return;
        }
        std::thread::spawn(|| unsafe {
            // Low-level hooks are delivered through the installing thread's message loop.
            SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(on_key),
                GetModuleHandleW(std::ptr::null()),
                0,
            );
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
    }

    unsafe extern "system" fn on_key(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
            if info.vkCode == HOLD_VK.load(Ordering::SeqCst) {
                match wparam as u32 {
                    // The hook runs before the async key state is updated, so this is the state
                    // before the event: down means key repeat.
                    WM_KEYDOWN | WM_SYSKEYDOWN => {
                        let was_down = unsafe { GetAsyncKeyState(info.vkCode as i32) } < 0;
                        pressed(was_down);
                    }
                    WM_KEYUP | WM_SYSKEYUP => hold_key().up(),
                    _ => {}
                }
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
    }

    fn hold_key() -> std::sync::MutexGuard<'static, HoldKey> {
        HOLD_KEY.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn pressed(was_down: bool) {
        let Some(press) = hold_key().down(was_down) else {
            return;
        };
        if let Some(app) = APP.get() {
            if super::super::overlay::game_focused(app) {
                use tauri::Emitter;
                let _ = app.emit("overlay://peek", ());
            }
        }
        std::thread::spawn(move || {
            std::thread::sleep(HOLD);
            if hold_key().still_held(press) {
                if let Some(app) = APP.get() {
                    if super::super::overlay::game_focused(app) {
                        let app2 = app.clone();
                        let _ = app.run_on_main_thread(move || super::open_at_cursor(&app2));
                    }
                }
            }
        });
    }
}
