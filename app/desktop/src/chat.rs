//! The assistant's chat window. The conversation and the tool loop live in exile_assistant.

use std::sync::{Arc, Mutex, OnceLock};

use exile_assistant::{Anthropic, ChatError, Conversation, Server, Setup, Stop, Usage};
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

use crate::settings;

pub const CHAT_LABEL: &str = "chat";
const USER_AGENT: &str = concat!(
    "ExileCompanion/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/muirch/exile-companion)"
);

#[derive(Default)]
pub struct ChatState {
    conversation: tokio::sync::Mutex<Conversation>,
    server: tokio::sync::Mutex<Option<Arc<Server>>>,
    task: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatView {
    pub has_key: bool,
    /// False in a build without exile-companion-mcp next to the app.
    pub available: bool,
    pub busy: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TurnOver {
    /// "done", "cut", "refused", "tooManySteps", "stopped" or "error".
    stop: &'static str,
    message: Option<String>,
    usage: Option<Usage>,
    dollars: Option<f64>,
}

fn stop_name(stop: Stop) -> &'static str {
    match stop {
        Stop::Done => "done",
        Stop::Cut => "cut",
        Stop::Refused => "refused",
        Stop::TooManySteps => "tooManySteps",
    }
}

mod key {
    use std::sync::OnceLock;

    const SERVICE: &str = "Exile Companion";
    const USER: &str = "anthropic-api-key";

    fn entry() -> Result<keyring_core::Entry, String> {
        static STORE: OnceLock<Result<(), String>> = OnceLock::new();
        STORE
            .get_or_init(|| {
                #[cfg(windows)]
                let store = windows_native_keyring_store::Store::new();
                #[cfg(target_os = "linux")]
                let store = zbus_secret_service_keyring_store::Store::new();
                #[cfg(not(any(windows, target_os = "linux")))]
                return Err("no credential store on this system".to_string());
                #[cfg(any(windows, target_os = "linux"))]
                store
                    .map(|s| keyring_core::set_default_store(s))
                    .map_err(|e| e.to_string())
            })
            .clone()?;
        keyring_core::Entry::new(SERVICE, USER).map_err(|e| e.to_string())
    }

    pub fn read() -> Option<String> {
        match entry().and_then(|e| e.get_password().map_err(|e| e.to_string())) {
            Ok(k) if !k.is_empty() => Some(k),
            Ok(_) => None,
            Err(e) => {
                log::info!("no API key: {e}");
                None
            }
        }
    }

    pub fn write(key: &str) -> Result<(), String> {
        entry()?
            .set_password(key)
            .map_err(|e| format!("Couldn't save the key in the system's credential store ({e})."))
    }

    pub fn forget() -> Result<(), String> {
        match entry()?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

// The keyring can wait on a D-Bus or credential manager call, so it stays off the main thread.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .expect("keyring task panicked")
}

pub async fn view(app: &AppHandle) -> ChatView {
    let has_key = blocking(|| key::read().is_some()).await;
    ChatView {
        has_key,
        available: crate::assistant::server_path().is_some(),
        busy: busy(app),
    }
}

pub async fn set_key(app: &AppHandle, value: String) -> Result<ChatView, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err("Paste the key first.".into());
    }
    blocking(move || key::write(&value)).await?;
    Ok(view(app).await)
}

pub async fn forget_key(app: &AppHandle) -> Result<ChatView, String> {
    blocking(key::forget).await?;
    Ok(view(app).await)
}

fn busy(app: &AppHandle) -> bool {
    let state = app.state::<ChatState>();
    let task = state.task.lock().unwrap_or_else(|e| e.into_inner());
    task.as_ref().is_some_and(|t| !t.inner().is_finished())
}

async fn server(app: &AppHandle) -> Result<Arc<Server>, ChatError> {
    let state = app.state::<ChatState>();
    let mut slot = state.server.lock().await;
    if let Some(s) = slot.as_ref().filter(|s| s.is_running()) {
        return Ok(s.clone());
    }
    let exe = crate::assistant::server_path().ok_or_else(|| {
        ChatError::Tools("exile-companion-mcp isn't installed with this build".into())
    })?;
    let started = Arc::new(Server::start(&exe, &[]).await?);
    log::info!("assistant tools started, {} tools", started.tools.len());
    *slot = Some(started.clone());
    Ok(started)
}

fn finish(app: &AppHandle, over: TurnOver) {
    let _ = app.emit_to(CHAT_LABEL, "chat://over", over);
}

async fn turn(app: AppHandle, question: String) {
    let Some(api_key) = blocking(key::read).await else {
        finish(
            &app,
            TurnOver {
                stop: "error",
                message: Some("Add your Anthropic API key in the settings first.".into()),
                usage: None,
                dollars: None,
            },
        );
        return;
    };
    let failed = |e: ChatError| {
        log::warn!("chat: {e}");
        TurnOver {
            stop: "error",
            message: Some(e.player_message()),
            usage: None,
            dollars: None,
        }
    };
    let server = match server(&app).await {
        Ok(s) => s,
        Err(e) => return finish(&app, failed(e)),
    };
    let config = settings::load(&app).unwrap_or_default().chat;
    let system = exile_assistant::system_prompt(&server.instructions);
    let setup = Setup {
        config,
        system: &system,
        tools: &server.tools,
    };
    let llm = Anthropic::new(api_key, USER_AGENT);
    let state = app.state::<ChatState>();
    let mut conversation = state.conversation.lock().await;
    let events = app.clone();
    let result = conversation
        .ask(&setup, &llm, server.as_ref(), &question, move |e| {
            let _ = events.emit_to(CHAT_LABEL, "chat://event", e);
        })
        .await;
    let over = match result {
        Ok(end) => {
            let dollars = end.usage.dollars(config.model);
            log::info!(
                "chat turn {:?} on {}: {:?}, about ${dollars:.3}",
                end.stop,
                config.model.id(),
                end.usage
            );
            TurnOver {
                stop: stop_name(end.stop),
                message: end.refusal,
                usage: Some(end.usage),
                dollars: Some(dollars),
            }
        }
        Err(e) => failed(e),
    };
    finish(&app, over);
}

pub fn send(app: &AppHandle, question: String) -> Result<(), String> {
    let question = question.trim().to_string();
    if question.is_empty() {
        return Ok(());
    }
    let state = app.state::<ChatState>();
    let mut task = state.task.lock().unwrap_or_else(|e| e.into_inner());
    if task.as_ref().is_some_and(|t| !t.inner().is_finished()) {
        return Err("Still answering the last question.".into());
    }
    *task = Some(tauri::async_runtime::spawn(turn(app.clone(), question)));
    Ok(())
}

/// The conversation keeps only finished turns, so dropping the task leaves it at the last answer.
pub fn stop(app: &AppHandle) {
    let state = app.state::<ChatState>();
    let running = state
        .task
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .filter(|t| !t.inner().is_finished());
    if let Some(t) = running {
        t.abort();
        finish(
            app,
            TurnOver {
                stop: "stopped",
                message: None,
                usage: None,
                dollars: None,
            },
        );
    }
}

pub async fn new_chat(app: &AppHandle) {
    stop(app);
    let state = app.state::<ChatState>();
    *state.conversation.lock().await = Conversation::default();
}

/// Right of the screen center, clear of the overlay at the bottom and the play field around the
/// character, unless the player moved it.
fn place(app: &AppHandle, window: &tauri::WebviewWindow) {
    let monitor = app
        .get_webview_window("overlay")
        .and_then(|w| w.current_monitor().ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten());
    let (Some(m), Ok(size)) = (monitor, window.outer_size()) else {
        return;
    };
    let (mp, ms) = (m.position(), m.size());
    let max_x = (ms.width as i32 - size.width as i32).max(0);
    let max_y = (ms.height as i32 - size.height as i32).max(0);
    let (x, y) = match settings::load(app).ok().and_then(|s| s.chat_anchor) {
        Some((fx, fy)) => (
            (fx * ms.width as f64) as i32,
            (fy * ms.height as f64) as i32,
        ),
        None => (max_x - ms.width as i32 / 40, ms.height as i32 / 8),
    };
    let _ = window.set_position(PhysicalPosition::new(
        mp.x + x.clamp(0, max_x),
        mp.y + y.clamp(0, max_y),
    ));
}

fn remember_place(app: &AppHandle, window: &tauri::WebviewWindow) {
    let (Ok(pos), Ok(Some(m))) = (window.outer_position(), window.current_monitor()) else {
        return;
    };
    let (mp, ms) = (m.position(), m.size());
    let fx = (pos.x - mp.x) as f64 / ms.width as f64;
    let fy = (pos.y - mp.y) as f64 / ms.height as f64;
    if let Ok(mut s) = settings::load(app) {
        s.chat_anchor = Some((fx.clamp(0.0, 1.0), fy.clamp(0.0, 1.0)));
        let _ = settings::save(app, &s);
    }
}

static PLACED: OnceLock<()> = OnceLock::new();

pub fn open(app: &AppHandle) {
    crate::menu::close(app);
    let Some(w) = app.get_webview_window(CHAT_LABEL) else {
        return;
    };
    if PLACED.set(()).is_ok() {
        place(app, &w);
    }
    let _ = w.show();
    let _ = w.unminimize();
    let _ = w.set_focus();
    let _ = app.emit_to(CHAT_LABEL, "chat://opened", ());
    #[cfg(target_os = "linux")]
    crate::menu::focus_window(crate::menu::CHAT_TITLE);
}

/// Hides the window and gives the keyboard back to the game. An answer still streaming goes on.
pub fn close(app: &AppHandle) {
    let Some(w) = app.get_webview_window(CHAT_LABEL) else {
        return;
    };
    if w.is_visible().unwrap_or(false) {
        remember_place(app, &w);
    }
    // Before hiding, while the chat still has focus. The game only gets it back from our window.
    crate::overlay::focus_game(app);
    let _ = w.hide();
}

pub fn toggle(app: &AppHandle) {
    let shown = app
        .get_webview_window(CHAT_LABEL)
        .is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
    if shown {
        close(app);
    } else {
        open(app);
    }
}
