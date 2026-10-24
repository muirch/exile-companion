use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager};

use crate::overlay;
use crate::settings;
use crate::state::AppState;

pub const MENU_LABEL: &str = "menu";
pub const SETTINGS_LABEL: &str = "settings";
#[cfg(target_os = "linux")]
pub const CHAT_TITLE: &[u8] = b"Exile Companion assistant";

/// Opens the quick menu with its top-left corner at the cursor, kept on the cursor's monitor.
#[cfg(any(windows, target_os = "linux"))]
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
    #[cfg(target_os = "linux")]
    x11::set_hold_key(name);
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = name;
}

/// Asks the window manager to focus one of our windows, with the time of the player's last key.
#[cfg(target_os = "linux")]
pub fn focus_window(title: &'static [u8]) {
    x11::focus(title);
}

/// Holding the menu key opens the menu. The hook only watches: every key event is passed on to
/// the game untouched.
pub fn watch_menu_hold(app: AppHandle) {
    #[cfg(windows)]
    win::watch(app);
    #[cfg(target_os = "linux")]
    x11::watch(app);
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = app;
}

/// What a press and a release of the menu key do, whichever way the platform reports them.
#[cfg(any(windows, target_os = "linux"))]
mod hold {
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;

    use exile_core::hold::HoldKey;
    use tauri::{AppHandle, Emitter};

    const HOLD: Duration = Duration::from_millis(450);

    pub static APP: OnceLock<AppHandle> = OnceLock::new();
    static HOLD_KEY: Mutex<HoldKey> = Mutex::new(HoldKey::new());

    fn hold_key() -> std::sync::MutexGuard<'static, HoldKey> {
        HOLD_KEY.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `was_down`: the key was already down, so this press is a key repeat.
    pub fn pressed(was_down: bool) {
        let Some(press) = hold_key().down(was_down) else {
            return;
        };
        if let Some(app) = APP.get() {
            if crate::overlay::game_focused(app) {
                let _ = app.emit("overlay://peek", ());
            }
        }
        std::thread::spawn(move || {
            std::thread::sleep(HOLD);
            if hold_key().still_held(press) {
                if let Some(app) = APP.get() {
                    if crate::overlay::game_focused(app) {
                        let app2 = app.clone();
                        let _ = app.run_on_main_thread(move || super::open_at_cursor(&app2));
                        #[cfg(target_os = "linux")]
                        super::x11::focus_menu();
                    }
                }
            }
        });
    }

    pub fn released() {
        hold_key().up();
    }
}

/// Raw XInput2 key events: they only report keys, every key still reaches the game. X sends
/// them while an X11 window has focus, which the game under Proton is.
#[cfg(target_os = "linux")]
mod x11 {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::time::Duration;

    use tauri::AppHandle;
    use x11rb::connection::Connection;
    use x11rb::protocol::xinput::{self, ConnectionExt as _};
    use x11rb::protocol::xproto::{
        Atom, AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, Window,
    };
    use x11rb::protocol::Event;
    use x11rb::rust_connection::RustConnection;

    // Physical keys, as on Windows: the key left of 1 is the menu key in any layout. These are
    // the evdev codes plus 8 that Xwayland uses.
    const TILDE_KEY: u32 = 49;
    const TAB_KEY: u32 = 23;

    static TAB: AtomicBool = AtomicBool::new(false);
    /// X server time of the last key event, the proof GNOME wants that the player asked.
    static KEY_TIME: AtomicU32 = AtomicU32::new(0);
    const MENU_TITLE: &[u8] = b"Exile Companion menu";

    pub fn set_hold_key(name: &str) {
        TAB.store(name == "Tab", Ordering::SeqCst);
    }

    pub fn watch(app: AppHandle) {
        if super::hold::APP.set(app).is_err() {
            return;
        }
        std::thread::spawn(|| {
            if let Err(e) = run() {
                log::info!("the menu key isn't watched: {e}");
            }
        });
    }

    fn run() -> Result<(), Box<dyn std::error::Error>> {
        let (conn, screen) = x11rb::connect(None)?;
        let root = conn.setup().roots[screen].root;
        conn.xinput_xi_query_version(2, 0)?.reply()?;
        conn.xinput_xi_select_events(
            root,
            &[xinput::EventMask {
                deviceid: xinput::Device::ALL_MASTER.into(),
                mask: vec![
                    xinput::XIEventMask::RAW_KEY_PRESS | xinput::XIEventMask::RAW_KEY_RELEASE,
                ],
            }],
        )?;
        conn.flush()?;
        let mut down = false;
        loop {
            let (key, press, time) = match conn.wait_for_event()? {
                Event::XinputRawKeyPress(e) => (e.detail, true, e.time),
                Event::XinputRawKeyRelease(e) => (e.detail, false, e.time),
                _ => continue,
            };
            // Any key counts as the player's action, also the chat's shortcut.
            KEY_TIME.store(time, Ordering::SeqCst);
            let watched = if TAB.load(Ordering::SeqCst) {
                TAB_KEY
            } else {
                TILDE_KEY
            };
            if key != watched {
                continue;
            }
            if press {
                super::hold::pressed(down);
            } else {
                super::hold::released();
            }
            down = press;
        }
    }

    /// GNOME keeps focus from a window that shows up without the player touching it, so the first
    /// click on the menu would only focus it. Asking for focus with the key's time, the way
    /// pagers and xdotool do, counts as the player's own action.
    pub fn focus_menu() {
        focus(MENU_TITLE);
    }

    pub fn focus(title: &'static [u8]) {
        let time = KEY_TIME.load(Ordering::SeqCst);
        std::thread::spawn(move || {
            if let Err(e) = activate(title, time) {
                log::info!("couldn't focus {}: {e}", String::from_utf8_lossy(title));
            }
        });
    }

    fn activate(title: &[u8], time: u32) -> Result<(), Box<dyn std::error::Error>> {
        let (conn, screen) = x11rb::connect(None)?;
        let root = conn.setup().roots[screen].root;
        let atom = |name: &[u8]| -> Result<Atom, Box<dyn std::error::Error>> {
            Ok(conn.intern_atom(false, name)?.reply()?.atom)
        };
        let (active, clients, pid, name, utf8) = (
            atom(b"_NET_ACTIVE_WINDOW")?,
            atom(b"_NET_CLIENT_LIST")?,
            atom(b"_NET_WM_PID")?,
            atom(b"_NET_WM_NAME")?,
            atom(b"UTF8_STRING")?,
        );
        // The window manager lists the window once it has mapped it, a moment after show().
        for _ in 0..25 {
            if let Some(window) = find(&conn, root, clients, pid, name, utf8, title)? {
                let event = ClientMessageEvent::new(32, window, active, [2, time, 0, 0, 0]);
                conn.send_event(
                    false,
                    root,
                    EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                    event,
                )?;
                conn.flush()?;
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Err("the window didn't show up".into())
    }

    fn find(
        conn: &RustConnection,
        root: Window,
        clients: Atom,
        pid: Atom,
        name: Atom,
        utf8: Atom,
        title: &[u8],
    ) -> Result<Option<Window>, Box<dyn std::error::Error>> {
        let list = conn
            .get_property(false, root, clients, AtomEnum::WINDOW, 0, 1024)?
            .reply()?;
        let Some(windows) = list.value32() else {
            return Ok(None);
        };
        for window in windows {
            let owner = conn
                .get_property(false, window, pid, AtomEnum::CARDINAL, 0, 1)?
                .reply()?;
            if owner.value32().and_then(|mut v| v.next()) != Some(std::process::id()) {
                continue;
            }
            let n = conn
                .get_property(false, window, name, utf8, 0, 64)?
                .reply()?;
            if n.value == title {
                return Ok(Some(window));
            }
        }
        Ok(None)
    }
}

#[cfg(windows)]
mod win {
    use std::sync::atomic::{AtomicU32, Ordering};

    use tauri::AppHandle;
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;

    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_OEM_3, VK_TAB};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
        KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    static HOLD_VK: AtomicU32 = AtomicU32::new(VK_OEM_3 as u32);

    pub fn set_hold_key(name: &str) {
        let vk = match name {
            "Tab" => VK_TAB,
            _ => VK_OEM_3,
        };
        HOLD_VK.store(u32::from(vk), Ordering::SeqCst);
    }

    pub fn watch(app: AppHandle) {
        if super::hold::APP.set(app).is_err() {
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
                        super::hold::pressed(was_down);
                    }
                    WM_KEYUP | WM_SYSKEYUP => super::hold::released(),
                    _ => {}
                }
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
    }
}
