use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow, Window,
};

use crate::settings::{self, Settings};
use crate::state::AppState;

const OVERLAY_LABEL: &str = "overlay";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Grabbed means the overlay takes the mouse (drag, buttons). Otherwise clicks go to the game.
pub fn set_grab(app: &AppHandle, grabbed: bool) {
    let state = app.state::<AppState>();
    let was = state.grabbed.swap(grabbed, Ordering::SeqCst);
    // Dragging is only possible while unlocked, so locking is when a new position is final.
    if was && !grabbed {
        save_anchor(app);
        // Moving took keyboard focus from the game. Hand it back so its keys work again.
        #[cfg(windows)]
        win::focus_game_if_own();
    }
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.set_ignore_cursor_events(!grabbed);
    }
    let _ = app.emit("overlay://grab-changed", grabbed);
}

/// Gives the keyboard back to the game when one of our windows took it. On Linux the window
/// manager does that when our window hides.
pub fn focus_game(app: &AppHandle) {
    #[cfg(windows)]
    win::focus_game_if_own();
    let _ = app;
}

pub fn toggle_grab(app: &AppHandle) {
    let grabbed = app.state::<AppState>().grabbed.load(Ordering::SeqCst);
    set_grab(app, !grabbed);
}

// In the gap between PoE's flask bar and skill bar, just above the experience bar. Higher up the
// plate covers the play field under the character, the game's centre banners and the stash and
// vendor windows.
pub const DEFAULT_ANCHOR: (f64, f64) = (0.5, 0.985);

fn monitor_of(window: &WebviewWindow) -> Option<Monitor> {
    window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
}

// Keeps the anchor point (bottom center) where it is, whatever size the window takes.
fn place(window: &WebviewWindow, anchor: (f64, f64)) -> tauri::Result<()> {
    let Some(monitor) = monitor_of(window) else {
        return Ok(());
    };
    let (mpos, msize) = (monitor.position(), monitor.size());
    let size = window.outer_size()?;
    let (w, h) = (size.width as i32, size.height as i32);
    let (mw, mh) = (msize.width as i32, msize.height as i32);
    let x = mpos.x + (anchor.0 * mw as f64) as i32 - w / 2;
    let y = mpos.y + (anchor.1 * mh as f64) as i32 - h;
    let x = x.clamp(mpos.x, mpos.x + (mw - w).max(0));
    let y = y.clamp(mpos.y, mpos.y + (mh - h).max(0));
    window.set_position(PhysicalPosition::new(x, y))
}

pub fn restore_anchor(app: &AppHandle, saved: &Settings) {
    if let Some(a) = saved.anchor {
        *lock(&app.state::<AppState>().anchor) = a;
    }
}

/// Sizes the window to the overlay content (CSS pixels) and re-places it on its anchor.
pub fn fit(app: &AppHandle, width: f64, height: f64) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(OVERLAY_LABEL) else {
        return Ok(());
    };
    let scale = window.scale_factor()?;
    let size = PhysicalSize::new(
        (width * scale).ceil().max(1.0) as u32,
        (height * scale).ceil().max(1.0) as u32,
    );
    if window.outer_size()? != size {
        window.set_size(size)?;
    }
    let anchor = *lock(&app.state::<AppState>().anchor);
    place(&window, anchor)
}

/// Remembers where the player dragged the overlay, as an anchor relative to the monitor.
pub fn on_moved(window: &Window) {
    if window.label() != OVERLAY_LABEL {
        return;
    }
    let app = window.app_handle();
    let state = app.state::<AppState>();
    if !state.grabbed.load(Ordering::SeqCst) {
        return;
    }
    let (Ok(pos), Ok(size), Some(monitor)) = (
        window.outer_position(),
        window.outer_size(),
        window.current_monitor().ok().flatten(),
    ) else {
        return;
    };
    let (mpos, msize) = (monitor.position(), monitor.size());
    let x = (pos.x - mpos.x) as f64 + size.width as f64 / 2.0;
    let bottom = (pos.y - mpos.y) as f64 + size.height as f64;
    *lock(&state.anchor) = (x / msize.width as f64, bottom / msize.height as f64);
}

fn save_anchor(app: &AppHandle) {
    let anchor = *lock(&app.state::<AppState>().anchor);
    if let Ok(mut s) = settings::load(app) {
        s.anchor = Some(anchor);
        let _ = settings::save(app, &s);
    }
}

pub fn snap_bottom_center(app: &AppHandle) -> tauri::Result<()> {
    *lock(&app.state::<AppState>().anchor) = DEFAULT_ANCHOR;
    save_anchor(app);
    match app.get_webview_window(OVERLAY_LABEL) {
        Some(window) => place(&window, DEFAULT_ANCHOR),
        None => Ok(()),
    }
}

pub fn game_focused(app: &AppHandle) -> bool {
    app.state::<AppState>().game_focused.load(Ordering::SeqCst)
}

// The overlay has no taskbar button, so the tray is the only way to reach it outside the game.
pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let grab = MenuItem::with_id(app, "grab", "Move overlay", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Exile Companion", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings, &grab, &quit])?;
    let mut tray = TrayIconBuilder::new()
        .tooltip("Exile Companion")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "grab" => {
                set_grab(app, true);
                // Without focus the next foreground change (the menu closing) would lock it again.
                if let Some(w) = app.get_webview_window(OVERLAY_LABEL) {
                    let _ = w.set_focus();
                }
            }
            "settings" => crate::menu::open_settings(app),
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
/// from the game. On Linux this needs the app to run as an X11 window. Elsewhere the overlay is
/// always visible.
pub fn watch_game_focus(app: AppHandle) {
    #[cfg(windows)]
    win::watch(app);
    #[cfg(target_os = "linux")]
    x11::watch(app);
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = app;
}

#[cfg(any(windows, target_os = "linux"))]
fn on_foreground_changed(app: &AppHandle, is_game: bool, is_own: bool) {
    let state = app.state::<AppState>();
    // Alt-tabbing away while unlocked locks again, so the overlay never eats desktop clicks.
    if !is_game && !is_own && state.grabbed.load(Ordering::SeqCst) {
        set_grab(app, false);
    }
    state.game_foreground.store(is_game, Ordering::SeqCst);
    let focused = is_game || is_own;
    if state.game_focused.swap(focused, Ordering::SeqCst) != focused {
        if is_game {
            // The game coming back to the front can end up above the overlay after a fast Alt-Tab.
            if let Some(w) = app.get_webview_window("overlay") {
                let _ = w.set_always_on_top(true);
            }
        }
        let _ = app.emit("overlay://game-focus", focused);
    }
}

#[cfg(windows)]
mod win {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::OnceLock;

    use tauri::{AppHandle, Manager};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetClassNameW, GetForegroundWindow, GetMessageW, GetWindowTextW,
        SetForegroundWindow, SetTimer, TranslateMessage, EVENT_SYSTEM_FOREGROUND, MSG,
        WINEVENT_OUTOFCONTEXT,
    };

    // The hook callback has no user data pointer.
    static APP: OnceLock<AppHandle> = OnceLock::new();
    static LAST_GAME: AtomicIsize = AtomicIsize::new(0);

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
            // A fast Alt-Tab can leave the task switcher as the last event, with no event for the
            // game coming back. Rechecking every half second catches that.
            SetTimer(std::ptr::null_mut(), 0, 500, Some(on_timer));
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
    }

    // The event's window can already be stale by the time it arrives. Ask for the current one.
    unsafe extern "system" fn on_event(
        _hook: HWINEVENTHOOK,
        _event: u32,
        _hwnd: HWND,
        _object: i32,
        _child: i32,
        _thread: u32,
        _time: u32,
    ) {
        report(GetForegroundWindow());
    }

    unsafe extern "system" fn on_timer(_hwnd: HWND, _msg: u32, _id: usize, _time: u32) {
        report(GetForegroundWindow());
    }

    fn is_own(app: &AppHandle, hwnd: HWND) -> bool {
        !hwnd.is_null()
            && app
                .webview_windows()
                .values()
                .filter_map(|w| w.hwnd().ok())
                .any(|h| h.0 == hwnd)
    }

    fn report(hwnd: HWND) {
        let Some(app) = APP.get() else {
            return;
        };
        // Menu and settings count as ours too, so the overlay doesn't vanish while they're open.
        let is_own = is_own(app, hwnd);
        let is_game = !is_own && is_game(hwnd);
        if is_game {
            LAST_GAME.store(hwnd as isize, Ordering::SeqCst);
        }
        super::on_foreground_changed(app, is_game, is_own);
    }

    /// Gives focus back to the game, but only while one of our windows has it: after an Alt-Tab
    /// away the player meant to leave.
    pub fn focus_game_if_own() {
        let Some(app) = APP.get() else {
            return;
        };
        let game = LAST_GAME.load(Ordering::SeqCst) as HWND;
        if !game.is_null() && is_own(app, unsafe { GetForegroundWindow() }) {
            unsafe { SetForegroundWindow(game) };
        }
    }

    fn is_game(hwnd: HWND) -> bool {
        if hwnd.is_null() {
            return false;
        }
        let mut buf = [0u16; 256];
        let len = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        let class = String::from_utf16_lossy(&buf[..len.max(0) as usize]);
        let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        let title = String::from_utf16_lossy(&buf[..len.max(0) as usize]);
        exile_core::game_window::is_game_window(&class, &title)
    }
}

/// The active window from the X server. GNOME sets it for X11 windows, and the game runs as one
/// under Proton. While a native Wayland window has focus it reads as none.
#[cfg(target_os = "linux")]
mod x11 {
    use tauri::AppHandle;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{
        Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt, EventMask, Window,
    };
    use x11rb::protocol::Event;
    use x11rb::rust_connection::RustConnection;

    type Error = Box<dyn std::error::Error>;

    struct Atoms {
        active: Atom,
        pid: Atom,
        name: Atom,
        utf8: Atom,
    }

    pub fn watch(app: AppHandle) {
        std::thread::spawn(move || {
            if let Err(e) = run(&app) {
                log::info!("game focus isn't tracked: {e}");
            }
        });
    }

    fn run(app: &AppHandle) -> Result<(), Error> {
        let (conn, screen) = x11rb::connect(None)?;
        let root = conn.setup().roots[screen].root;
        let atom = |name: &[u8]| -> Result<Atom, Error> {
            Ok(conn.intern_atom(false, name)?.reply()?.atom)
        };
        let atoms = Atoms {
            active: atom(b"_NET_ACTIVE_WINDOW")?,
            pid: atom(b"_NET_WM_PID")?,
            name: atom(b"_NET_WM_NAME")?,
            utf8: atom(b"UTF8_STRING")?,
        };
        conn.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )?;
        conn.flush()?;
        report(app, &conn, root, &atoms);
        loop {
            if let Event::PropertyNotify(e) = conn.wait_for_event()? {
                if e.atom == atoms.active {
                    report(app, &conn, root, &atoms);
                }
            }
        }
    }

    fn report(app: &AppHandle, conn: &RustConnection, root: Window, atoms: &Atoms) {
        // A window can close between the change and our question about it. Then it's not the game.
        let (is_game, is_own) = active(conn, root, atoms).unwrap_or((false, false));
        super::on_foreground_changed(app, is_game, is_own);
    }

    fn active(conn: &RustConnection, root: Window, atoms: &Atoms) -> Result<(bool, bool), Error> {
        let property = |window: Window, atom: Atom, kind: Atom| -> Result<Vec<u8>, Error> {
            Ok(conn
                .get_property(false, window, atom, kind, 0, 256)?
                .reply()?
                .value)
        };
        let first = |bytes: Vec<u8>| {
            bytes
                .get(..4)
                .map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
        };
        let Some(window) =
            first(property(root, atoms.active, AtomEnum::WINDOW.into())?).filter(|w| *w != 0)
        else {
            return Ok((false, false));
        };
        let pid = first(property(window, atoms.pid, AtomEnum::CARDINAL.into())?);
        // Menu and settings count as ours too, so the overlay doesn't vanish while they're open.
        if pid == Some(std::process::id()) {
            return Ok((false, true));
        }
        let class = property(window, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into())?;
        let title = property(window, atoms.name, atoms.utf8)?;
        let title = String::from_utf8_lossy(&title);
        // WM_CLASS is the instance name and the class name, each ending in a NUL.
        let is_game = String::from_utf8_lossy(&class)
            .split('\0')
            .chain(std::iter::once(""))
            .any(|c| exile_core::game_window::is_game_window(c, &title));
        Ok((is_game, false))
    }
}
