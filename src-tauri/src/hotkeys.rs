use std::sync::atomic::Ordering;

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
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            menu_hold: "Escape".into(),
            move_overlay: "Alt+KeyM".into(),
            route_prev: "Alt+ArrowLeft".into(),
            route_next: "Alt+ArrowRight".into(),
            relog: "F9".into(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Action {
    MoveOverlay,
    RoutePrev,
    RouteNext,
    Relog,
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
    ];
    // An unused binding stays free for the game.
    if saved.relog_enabled {
        bindings.push(("relog", keys.relog.as_str(), Action::Relog));
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
        Action::Relog if in_game => chat_command(app, "/exit"),
        Action::Relog => {}
    }
}

/// Sends one chat command, the way Awakened PoE Trade does: release the shortcut's keys, put the
/// text on the clipboard, Enter, Ctrl+V, Enter, then put the old clipboard back.
fn chat_command(app: &AppHandle, text: &str) {
    #[cfg(windows)]
    {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        let clipboard = app.clipboard();
        let previous = clipboard.read_text().ok();
        if clipboard.write_text(text).is_err() {
            return;
        }
        win::release_modifiers();
        win::tap(win::VK_RETURN, false);
        win::tap(win::VK_V, true);
        win::tap(win::VK_RETURN, false);
        let app = app.clone();
        std::thread::spawn(move || {
            // The game reads the clipboard when it handles Ctrl+V, a moment after we sent it.
            std::thread::sleep(std::time::Duration::from_millis(250));
            if let Some(prev) = previous {
                let _ = app.clipboard().write_text(prev);
            }
        });
    }
    #[cfg(not(windows))]
    let _ = (app, text);
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
        VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };

    pub use windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_RETURN;
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

    // Alt still held from Alt+something would turn our Enter into the game's Alt+Enter.
    pub fn release_modifiers() {
        for vk in [VK_MENU, VK_CONTROL, VK_SHIFT, VK_LWIN, VK_RWIN] {
            if unsafe { GetAsyncKeyState(i32::from(vk)) } as u16 & 0x8000 != 0 {
                send(&[key(vk, true)]);
            }
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
