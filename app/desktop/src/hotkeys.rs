use std::sync::atomic::Ordering;
use std::sync::{LazyLock, Mutex};

use exile_core::vendor_search::SearchCycle;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::runtime::{self, RouteMove};
use crate::settings;
use crate::state::AppState;
use crate::{menu, overlay};

/// Shortcuts in global-hotkey's syntax ("Alt+ArrowLeft", "F9"), except `menu_hold`, which is a
/// single key watched for a long press.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Hotkeys {
    pub menu_hold: String,
    pub move_overlay: String,
    pub route_prev: String,
    pub route_next: String,
    pub relog: String,
    pub open_chat: String,
    pub vendor_search: String,
    pub hideout: String,
    pub item_check: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            menu_hold: "Backquote".into(),
            move_overlay: "Alt+KeyM".into(),
            route_prev: "Alt+ArrowLeft".into(),
            route_next: "Alt+ArrowRight".into(),
            relog: "F9".into(),
            open_chat: "Alt+KeyA".into(),
            vendor_search: "Alt+KeyG".into(),
            hideout: "F5".into(),
            // Not Ctrl+D: trade tools take it, and players run them alongside.
            item_check: "Alt+KeyD".into(),
        }
    }
}

// Esc isn't offered: the game acts on it first (closes panels, or quits from the main screen)
// and takes focus back from the menu.
const HOLD_KEYS: [&str; 2] = ["Backquote", "Tab"];

impl Hotkeys {
    /// Replaces a hold key that is no longer offered with the default.
    pub fn normalize(&mut self) {
        if !HOLD_KEYS.contains(&self.menu_hold.as_str()) {
            self.menu_hold = Self::default().menu_hold;
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Action {
    MoveOverlay,
    RoutePrev,
    RouteNext,
    Relog,
    OpenChat,
    VendorSearch,
    Hideout,
    ItemCheck,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindError {
    pub action: String,
    pub shortcut: String,
    pub error: String,
}

/// Registers every shortcut from settings, replacing the previous set. Bindings that fail
/// (bad syntax, taken by another program) come back so the settings window can show them.
pub fn register_all(app: &AppHandle) -> Vec<BindError> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let saved = settings::load(app).unwrap_or_default();
    let keys = &saved.hotkeys;
    menu::set_hold_key(&keys.menu_hold);

    let mut bindings = vec![
        (
            "moveOverlay",
            keys.move_overlay.as_str(),
            Action::MoveOverlay,
        ),
        ("routePrev", keys.route_prev.as_str(), Action::RoutePrev),
        ("routeNext", keys.route_next.as_str(), Action::RouteNext),
        ("openChat", keys.open_chat.as_str(), Action::OpenChat),
    ];
    // An unused binding stays free for the game.
    if saved.relog_enabled {
        bindings.push(("relog", keys.relog.as_str(), Action::Relog));
    }
    if saved.hideout_enabled {
        bindings.push(("hideout", keys.hideout.as_str(), Action::Hideout));
    }
    if saved.item_check_enabled {
        bindings.push(("itemCheck", keys.item_check.as_str(), Action::ItemCheck));
    }
    if saved.vendor_search_enabled {
        bindings.push((
            "vendorSearch",
            keys.vendor_search.as_str(),
            Action::VendorSearch,
        ));
    }

    let mut errors = Vec::new();
    for (name, shortcut, action) in bindings {
        if shortcut.is_empty() {
            continue;
        }
        let result = gs.on_shortcut(shortcut, move |app, _s, event| {
            if event.state() == ShortcutState::Pressed {
                run(app, action);
            }
        });
        if let Err(e) = result {
            errors.push(BindError {
                action: name.into(),
                shortcut: shortcut.into(),
                error: e.to_string(),
            });
        }
    }
    errors
}

pub fn pause(app: &AppHandle) {
    let _ = app.global_shortcut().unregister_all();
}

fn run(app: &AppHandle, action: Action) {
    let in_game = app
        .state::<AppState>()
        .game_foreground
        .load(Ordering::SeqCst);
    match action {
        Action::MoveOverlay => overlay::toggle_grab(app),
        Action::RoutePrev => {
            runtime::move_route(app, RouteMove::Prev);
        }
        Action::RouteNext => {
            runtime::move_route(app, RouteMove::Next);
        }
        // Only ever into the game: typing a chat command into another window would be wrong.
        Action::Relog if in_game => into_game(app, "/exit".into(), Keys::ChatCommand),
        Action::Relog => {}
        Action::Hideout if in_game => into_game(app, "/hideout".into(), Keys::ChatCommand),
        Action::Hideout => {}
        Action::ItemCheck if in_game => crate::item::check(app),
        Action::ItemCheck => {}
        Action::OpenChat => crate::chat::toggle(app),
        Action::VendorSearch if in_game => vendor_search(app),
        Action::VendorSearch => {}
    }
}

static SEARCH: LazyLock<Mutex<SearchCycle>> = LazyLock::new(Default::default);

/// Types the search string for the gems to buy here into the game's search field. The game
/// highlights them in the open vendor window. A list too long for one string takes more presses.
fn vendor_search(app: &AppHandle) {
    if SENDING.load(Ordering::SeqCst) {
        return;
    }
    let strings = runtime::vendor_search_strings(app);
    let press = SEARCH
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .press(strings, std::time::Instant::now());
    if let Some(press) = press {
        into_game(app, press.text, Keys::Search);
    }
}

#[derive(Clone, Copy)]
enum Keys {
    /// Enter, Ctrl+V, Enter: one chat command.
    ChatCommand,
    /// Ctrl+F, Delete, Ctrl+V, Enter: one search in the open stash or vendor window. Ctrl+F
    /// selects what the field already holds, so Delete clears it.
    Search,
}

/// Set from a key press until the game has had our keys, so key repeat or a second press can't
/// send twice.
static SENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One action per key press: wait until the
/// player lets go of the shortcut's modifiers (we never send key-ups for keys they hold), check
/// the game is still in front, paste the text through the clipboard, then put the old clipboard
/// back and ignore the key for a moment.
fn into_game(app: &AppHandle, text: String, keys: Keys) {
    #[cfg(any(windows, target_os = "linux"))]
    {
        use std::time::Duration;
        use tauri_plugin_clipboard_manager::ClipboardExt;

        if SENDING.swap(true, Ordering::SeqCst) {
            return;
        }
        let app = app.clone();
        std::thread::spawn(move || {
            let in_game = || {
                app.state::<AppState>()
                    .game_foreground
                    .load(Ordering::SeqCst)
            };
            if os::wait_modifiers_up(Duration::from_secs(1)) && in_game() {
                let clipboard = app.clipboard();
                let previous = clipboard.read_text().ok();
                if clipboard.write_text(text).is_ok() {
                    match keys {
                        Keys::ChatCommand => {
                            os::tap(os::VK_RETURN, false);
                            os::tap(os::VK_V, true);
                            os::tap(os::VK_RETURN, false);
                        }
                        Keys::Search => {
                            os::tap(os::VK_F, true);
                            // The field takes a moment to get focus.
                            std::thread::sleep(Duration::from_millis(100));
                            os::tap(os::VK_DELETE, false);
                            os::tap(os::VK_V, true);
                            os::tap(os::VK_RETURN, false);
                        }
                    }
                    // The game reads the clipboard when it handles Ctrl+V, a moment after we sent it.
                    std::thread::sleep(Duration::from_millis(250));
                    if let Some(prev) = previous {
                        let _ = app.clipboard().write_text(prev);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(200));
            SENDING.store(false, Ordering::SeqCst);
        });
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = (app, text, keys);
}

/// Ctrl+C over the item under the cursor: Ctrl goes down
/// only if the player isn't holding it, and the shortcut's other modifiers stay as they are.
/// Copying an item is the game's own text copy, not a game action.
pub fn copy_item() {
    #[cfg(any(windows, target_os = "linux"))]
    os::copy();
}

#[cfg(windows)]
use win as os;
#[cfg(target_os = "linux")]
use xtest as os;

#[cfg(windows)]
mod win {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
        VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };

    pub use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_DELETE, VK_RETURN};
    const VK_C: u16 = 0x43;
    pub const VK_F: u16 = 0x46;
    pub const VK_V: u16 = 0x56;

    fn key(vk: u16, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn send(inputs: &[INPUT]) {
        unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
        }
    }

    // Alt still held from Alt+something would turn our Enter into the game's Alt+Enter, and Alt
    // with our Ctrl+F into Ctrl+Alt+F.
    pub fn wait_modifiers_up(timeout: std::time::Duration) -> bool {
        let start = std::time::Instant::now();
        loop {
            let held = [VK_MENU, VK_CONTROL, VK_SHIFT, VK_LWIN, VK_RWIN]
                .iter()
                .any(|&vk| unsafe { GetAsyncKeyState(i32::from(vk)) } as u16 & 0x8000 != 0);
            if !held {
                return true;
            }
            if start.elapsed() >= timeout {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    pub fn copy() {
        let ctrl_held = unsafe { GetAsyncKeyState(i32::from(VK_CONTROL)) } as u16 & 0x8000 != 0;
        if ctrl_held {
            send(&[key(VK_C, false), key(VK_C, true)]);
        } else {
            tap(VK_C, true);
        }
    }

    pub fn tap(vk: u16, with_ctrl: bool) {
        if with_ctrl {
            send(&[
                key(VK_CONTROL, false),
                key(vk, false),
                key(vk, true),
                key(VK_CONTROL, true),
            ]);
        } else {
            send(&[key(vk, false), key(vk, true)]);
        }
    }
}

/// The same keys through XTest. The game under Proton is an X11 window, and XTest's fake events
/// reach it like the player's own. Keycodes are the standard X ones the menu key watcher uses.
#[cfg(target_os = "linux")]
mod xtest {
    use std::time::{Duration, Instant};

    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt as _, KEY_PRESS_EVENT, KEY_RELEASE_EVENT};
    use x11rb::protocol::xtest::ConnectionExt as _;
    use x11rb::rust_connection::RustConnection;

    pub const VK_RETURN: u8 = 36;
    pub const VK_DELETE: u8 = 119;
    pub const VK_F: u8 = 41;
    pub const VK_V: u8 = 55;
    const VK_C: u8 = 54;
    const CONTROL_L: u8 = 37;
    // Alt, Control, Shift and Super, left and right.
    const MODIFIERS: [u8; 8] = [64, 108, 37, 105, 50, 62, 133, 134];

    fn connect() -> Option<RustConnection> {
        x11rb::connect(None).ok().map(|(conn, _)| conn)
    }

    fn held(conn: &RustConnection, keys: &[u8]) -> bool {
        let Some(map) = conn.query_keymap().ok().and_then(|c| c.reply().ok()) else {
            return false;
        };
        keys.iter()
            .any(|&k| map.keys[usize::from(k / 8)] & (1 << (k % 8)) != 0)
    }

    fn send(conn: &RustConnection, events: &[(u8, bool)]) {
        for &(key, down) in events {
            let kind = if down {
                KEY_PRESS_EVENT
            } else {
                KEY_RELEASE_EVENT
            };
            let _ = conn.xtest_fake_input(kind, key, x11rb::CURRENT_TIME, x11rb::NONE, 0, 0, 0);
        }
        let _ = conn.flush();
    }

    // Alt still held from Alt+something would turn our Enter into the game's Alt+Enter.
    pub fn wait_modifiers_up(timeout: Duration) -> bool {
        let Some(conn) = connect() else {
            return false;
        };
        let start = Instant::now();
        while held(&conn, &MODIFIERS) {
            if start.elapsed() >= timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }

    pub fn copy() {
        let Some(conn) = connect() else {
            return;
        };
        if held(&conn, &[CONTROL_L, 105]) {
            send(&conn, &[(VK_C, true), (VK_C, false)]);
        } else {
            tap_on(&conn, VK_C, true);
        }
    }

    fn tap_on(conn: &RustConnection, key: u8, with_ctrl: bool) {
        if with_ctrl {
            send(
                conn,
                &[
                    (CONTROL_L, true),
                    (key, true),
                    (key, false),
                    (CONTROL_L, false),
                ],
            );
        } else {
            send(conn, &[(key, true), (key, false)]);
        }
    }

    pub fn tap(key: u8, with_ctrl: bool) {
        if let Some(conn) = connect() {
            tap_on(&conn, key, with_ctrl);
        }
    }
}
