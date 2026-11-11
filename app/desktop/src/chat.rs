//! The assistant's chat window. The conversation and the tool loop live in exile_assistant.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use exile_assistant::history::{ChatStore, ChatSummary, SavedChat, SavedExchange, Warmth};
use exile_assistant::{
    Anthropic, ChatError, ChatEvent, Config, Conversation, Dialect, Llm, OpenAi, Provider, Server,
    Setup, Stop, Usage,
};
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition};

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
    /// Counts openings of the window, so a pending unload knows the chat came back.
    opened: AtomicU64,
    /// The chat being written to its file. Its id also names the conversation to OpenRouter.
    chat: Mutex<Option<SavedChat>>,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn store(app: &AppHandle) -> Option<ChatStore> {
    let dir = app.path().app_data_dir().ok()?;
    Some(ChatStore::new(dir.join("chats")))
}

/// The current chat's id, starting a chat when there is none.
fn chat_id(app: &AppHandle, config: &Config) -> String {
    let state = app.state::<ChatState>();
    let mut chat = state.chat.lock().unwrap_or_else(|e| e.into_inner());
    chat.get_or_insert_with(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        SavedChat::new(
            format!("chat-{nanos:x}"),
            now(),
            config.provider,
            config.model_label(),
        )
    })
    .id
    .clone()
}

fn dialect_of(provider: Provider) -> Dialect {
    match provider {
        Provider::Anthropic => Dialect::Anthropic,
        Provider::OpenRouter | Provider::Custom => Dialect::OpenAi,
    }
}

/// The answer's text as the chat window puts it together.
fn add_to_answer(answer: &mut String, event: &ChatEvent) {
    match event {
        ChatEvent::Text { text } => answer.push_str(text),
        ChatEvent::Paragraph if !answer.is_empty() && !answer.ends_with("\n\n") => {
            answer.push_str("\n\n")
        }
        _ => {}
    }
}

/// Which providers have a key stored.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Keys {
    pub anthropic: bool,
    pub open_router: bool,
    pub custom: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatView {
    pub keys: Keys,
    /// False in a build without exile-companion-mcp next to the app.
    pub available: bool,
    pub busy: bool,
    /// What the player still has to set before the picked provider can answer.
    pub missing: Option<String>,
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TurnOver {
    /// "done", "cut", "refused", "tooManySteps", "empty", "stopped" or "error".
    stop: &'static str,
    message: Option<String>,
    usage: Option<Usage>,
    dollars: Option<f64>,
    /// The model that answered, to tell answers apart while trying several.
    model: Option<String>,
}

impl TurnOver {
    fn bare(stop: &'static str, message: Option<String>) -> Self {
        Self {
            stop,
            message,
            usage: None,
            dollars: None,
            model: None,
        }
    }
}

fn stop_name(stop: Stop) -> &'static str {
    match stop {
        Stop::Done => "done",
        Stop::Cut => "cut",
        Stop::Refused => "refused",
        Stop::TooManySteps => "tooManySteps",
        Stop::Empty => "empty",
    }
}

mod key {
    use std::sync::OnceLock;

    use exile_assistant::Provider;

    const SERVICE: &str = "Exile Companion";

    fn user(provider: Provider) -> &'static str {
        match provider {
            Provider::Anthropic => "anthropic-api-key",
            Provider::OpenRouter => "openrouter-api-key",
            Provider::Custom => "custom-api-key",
        }
    }

    fn entry(provider: Provider) -> Result<keyring_core::Entry, String> {
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
        keyring_core::Entry::new(SERVICE, user(provider)).map_err(|e| e.to_string())
    }

    pub fn read(provider: Provider) -> Option<String> {
        match entry(provider).and_then(|e| e.get_password().map_err(|e| e.to_string())) {
            Ok(k) if !k.is_empty() => Some(k),
            Ok(_) => None,
            Err(e) => {
                log::info!("no {} key: {e}", provider.name());
                None
            }
        }
    }

    pub fn write(provider: Provider, key: &str) -> Result<(), String> {
        entry(provider)?
            .set_password(key)
            .map_err(|e| format!("Couldn't save the key in the system's credential store ({e})."))
    }

    pub fn forget(provider: Provider) -> Result<(), String> {
        match entry(provider)?.delete_credential() {
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

/// A custom server, often a local one, may not need a key.
fn missing(config: &Config, has_key: bool) -> Option<String> {
    if let Some(m) = config.missing() {
        return Some(m.into());
    }
    match config.provider {
        Provider::Custom => None,
        p if !has_key => Some(format!("Add your {} API key in the settings.", p.name())),
        _ => None,
    }
}

pub async fn view(app: &AppHandle) -> ChatView {
    let keys = blocking(|| Keys {
        anthropic: key::read(Provider::Anthropic).is_some(),
        open_router: key::read(Provider::OpenRouter).is_some(),
        custom: key::read(Provider::Custom).is_some(),
    })
    .await;
    let config = settings::load(app).unwrap_or_default().chat;
    let has_key = match config.provider {
        Provider::Anthropic => keys.anthropic,
        Provider::OpenRouter => keys.open_router,
        Provider::Custom => keys.custom,
    };
    ChatView {
        missing: missing(&config, has_key),
        model: config.model_label(),
        keys,
        available: crate::assistant::server_path().is_some(),
        busy: busy(app),
    }
}

pub async fn set_key(
    app: &AppHandle,
    provider: Provider,
    value: String,
) -> Result<ChatView, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err("Paste the key first.".into());
    }
    blocking(move || key::write(provider, &value)).await?;
    Ok(view(app).await)
}

pub async fn forget_key(app: &AppHandle, provider: Provider) -> Result<ChatView, String> {
    blocking(move || key::forget(provider)).await?;
    Ok(view(app).await)
}

/// `ollama` says the custom server is Ollama, the one that takes an effort.
fn model(
    config: &Config,
    api_key: Option<String>,
    ollama: bool,
    session: &str,
) -> Box<dyn Llm + Send> {
    let key = api_key.unwrap_or_default();
    match config.provider {
        Provider::Anthropic => {
            Box::new(Anthropic::new(key, config.model, config.effort, USER_AGENT))
        }
        Provider::OpenRouter => Box::new(
            OpenAi::openrouter(key, config.router_model.clone(), config.effort, USER_AGENT)
                .with_session(session),
        ),
        Provider::Custom => Box::new(OpenAi::custom(
            &config.custom_url,
            Some(key),
            config.custom_model.clone(),
            ollama.then_some(config.effort),
            USER_AGENT,
        )),
    }
}

fn busy(app: &AppHandle) -> bool {
    let state = app.state::<ChatState>();
    let task = state.task.lock().unwrap_or_else(|e| e.into_inner());
    task.as_ref().is_some_and(|t| !t.inner().is_finished())
}

pub(crate) async fn server(app: &AppHandle) -> Result<Arc<Server>, ChatError> {
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

/// What to change next, straight from the tools: no model, no key, nothing billed.
pub async fn next_card(app: &AppHandle) -> exile_assistant::NextCard {
    use exile_assistant::Tools;
    let failed = |message: String| exile_assistant::NextCard {
        missing: Some(message),
        ..Default::default()
    };
    let server = match server(app).await {
        Ok(s) => s,
        Err(e) => return failed(e.player_message(Provider::Anthropic)),
    };
    let out = server.call("what_next", serde_json::json!({})).await;
    if out.is_error {
        log::warn!("what_next: {}", out.text);
        return failed(out.text);
    }
    match serde_json::from_str(&out.text) {
        Ok(v) => exile_assistant::from_what_next(&v),
        Err(e) => failed(format!("The tools answered something unreadable ({e}).")),
    }
}

fn finish(app: &AppHandle, over: TurnOver) {
    let _ = app.emit_to(CHAT_LABEL, "chat://over", over);
}

async fn turn(app: AppHandle, question: String) {
    let config = settings::load(&app).unwrap_or_default().chat;
    let provider = config.provider;
    let api_key = blocking(move || key::read(provider)).await;
    if let Some(m) = missing(&config, api_key.is_some()) {
        return finish(&app, TurnOver::bare("error", Some(m)));
    }
    let failed = |e: ChatError| {
        log::warn!("chat: {e}");
        TurnOver::bare("error", Some(e.player_message(provider)))
    };
    let server = match server(&app).await {
        Ok(s) => s,
        Err(e) => return finish(&app, failed(e)),
    };
    let system = exile_assistant::system_prompt(&server.instructions);
    let setup = Setup {
        system: &system,
        tools: &server.tools,
    };
    let ollama = match local_ollama(&app) {
        Some((url, name)) => {
            blocking(move || exile_assistant::Ollama::detect(&url, &name, USER_AGENT).is_some())
                .await
        }
        None => false,
    };
    let id = chat_id(&app, &config);
    let llm = model(&config, api_key, ollama, &id);
    let label = config.model_label();
    let state = app.state::<ChatState>();
    let mut conversation = state.conversation.lock().await;
    let events = app.clone();
    let answer = Arc::new(Mutex::new(String::new()));
    let written = answer.clone();
    let result = conversation
        .ask(&setup, llm.as_ref(), server.as_ref(), &question, move |e| {
            add_to_answer(&mut written.lock().unwrap_or_else(|e| e.into_inner()), &e);
            let _ = events.emit_to(CHAT_LABEL, "chat://event", e);
        })
        .await;
    let over = match result {
        Ok(end) => {
            log::info!(
                "chat turn {:?} on {} {label}: {:?}",
                end.stop,
                provider.name(),
                end.usage
            );
            TurnOver {
                stop: stop_name(end.stop),
                message: end.refusal,
                usage: Some(end.usage),
                dollars: end.usage.cost,
                model: Some(label),
            }
        }
        Err(e) => failed(e),
    };
    let answer = std::mem::take(&mut *answer.lock().unwrap_or_else(|e| e.into_inner()));
    let exchange = SavedExchange {
        question,
        answer,
        stop: over.stop.to_string(),
        message: over.message.clone(),
        dollars: over.dollars,
        model: over.model.clone(),
    };
    save_turn(&app, exchange, &conversation);
    finish(&app, over);
}

fn save_turn(app: &AppHandle, exchange: SavedExchange, conversation: &Conversation) {
    let state = app.state::<ChatState>();
    let mut chat = state.chat.lock().unwrap_or_else(|e| e.into_inner());
    let Some(chat) = chat.as_mut() else {
        return;
    };
    chat.record(exchange, conversation, now());
    if let Some(store) = store(app) {
        if let Err(e) = store.save(chat) {
            log::warn!("could not save chat {}: {e}", chat.id);
        }
    }
}

pub fn history(app: &AppHandle) -> Vec<ChatSummary> {
    store(app).map(|s| s.list()).unwrap_or_default()
}

/// A saved chat as the window shows it before the player goes on with it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedChat {
    pub id: String,
    pub title: String,
    pub exchanges: Vec<SavedExchange>,
    pub warmth: Warmth,
    /// Why the picked provider can't go on with it, if it can't.
    pub blocked: Option<String>,
}

pub fn open_saved(app: &AppHandle, id: &str) -> Result<OpenedChat, String> {
    let store = store(app).ok_or("No folder for chats.")?;
    let chat = store.load(id).map_err(|e| e.to_string())?;
    let config = settings::load(app).unwrap_or_default().chat;
    let blocked = (!chat.continues_with(dialect_of(config.provider))).then(|| {
        let with = match chat.provider {
            Provider::Anthropic => "Anthropic",
            Provider::OpenRouter => "OpenRouter",
            Provider::Custom => "your own server",
        };
        format!(
            "This chat was with {with}, which keeps the conversation in another shape. Switch to it in the settings to go on with this chat."
        )
    });
    let claude = (config.provider == Provider::Anthropic).then_some(config.model);
    Ok(OpenedChat {
        id: chat.id.clone(),
        title: chat.title.clone(),
        warmth: chat.warmth(now(), claude),
        exchanges: chat.exchanges,
        blocked,
    })
}

/// Makes a saved chat the current one, so the next question goes on with it.
pub async fn resume(app: &AppHandle, id: &str) -> Result<Vec<SavedExchange>, String> {
    let store = store(app).ok_or("No folder for chats.")?;
    let chat = store.load(id).map_err(|e| e.to_string())?;
    stop(app);
    let state = app.state::<ChatState>();
    *state.conversation.lock().await = chat.conversation();
    let exchanges = chat.exchanges.clone();
    *state.chat.lock().unwrap_or_else(|e| e.into_inner()) = Some(chat);
    Ok(exchanges)
}

pub async fn delete_saved(app: &AppHandle, id: &str) -> Result<(), String> {
    let current = app
        .state::<ChatState>()
        .chat
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|c| c.id == id);
    if current {
        new_chat(app).await;
    }
    let store = store(app).ok_or("No folder for chats.")?;
    store.delete(id).map_err(|e| e.to_string())
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
        finish(app, TurnOver::bare("stopped", None));
    }
}

pub async fn new_chat(app: &AppHandle) {
    stop(app);
    let state = app.state::<ChatState>();
    *state.conversation.lock().await = Conversation::default();
    *state.chat.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

// The window's size in tauri.conf.json, at 1080p.
const CHAT_SIZE: (f64, f64) = (420.0, 560.0);
const CHAT_MIN_SIZE: (f64, f64) = (340.0, 320.0);

/// PoE scales its UI with the monitor height, and the overlay does the same, so the chat follows:
/// 100% at 1080p.
pub(crate) fn screen_zoom(logical_height: f64) -> f64 {
    (logical_height / 1080.0).clamp(0.75, 2.0)
}

/// Scaled to the monitor, right of the screen center, clear of the overlay at the bottom and the
/// play field around the character, unless the player moved it.
fn place(app: &AppHandle, window: &tauri::WebviewWindow) {
    let monitor = app
        .get_webview_window("overlay")
        .and_then(|w| w.current_monitor().ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten());
    let Some(m) = monitor else {
        return;
    };
    let (mp, ms) = (m.position(), m.size());
    let zoom = screen_zoom(ms.height as f64 / m.scale_factor());
    let _ = window.set_zoom(zoom);
    let _ = window.set_min_size(Some(LogicalSize::new(
        CHAT_MIN_SIZE.0 * zoom,
        CHAT_MIN_SIZE.1 * zoom,
    )));
    let _ = window.set_size(LogicalSize::new(CHAT_SIZE.0 * zoom, CHAT_SIZE.1 * zoom));
    let px = |v: f64| (v * zoom * m.scale_factor()) as i32;
    let max_x = (ms.width as i32 - px(CHAT_SIZE.0)).max(0);
    let max_y = (ms.height as i32 - px(CHAT_SIZE.1)).max(0);
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
    app.state::<ChatState>()
        .opened
        .fetch_add(1, Ordering::SeqCst);
    warm_local_model(app);
}

// The model loads while the player types the question. Half an hour covers a window left open.
const KEEP_WHILE_OPEN: Duration = Duration::from_secs(30 * 60);
const UNLOAD_AFTER: Duration = Duration::from_secs(60);

/// Ollama's address and model when the chat runs on Ollama. Other servers manage memory alone.
fn local_ollama(app: &AppHandle) -> Option<(String, String)> {
    let c = settings::load(app).ok()?.chat;
    (c.provider == Provider::Custom && c.missing().is_none())
        .then_some((c.custom_url, c.custom_model))
}

fn warm_local_model(app: &AppHandle) {
    let Some((url, model)) = local_ollama(app) else {
        return;
    };
    tauri::async_runtime::spawn_blocking(move || {
        let Some(ollama) = exile_assistant::Ollama::detect(&url, &model, USER_AGENT) else {
            return;
        };
        let started = std::time::Instant::now();
        match ollama.load(KEEP_WHILE_OPEN) {
            Ok(()) => log::info!("loaded {model} in {:.1}s", started.elapsed().as_secs_f64()),
            Err(e) => log::info!("couldn't load {model}: {e}"),
        }
    });
}

/// A minute after the chat closes the game gets the video memory back, unless the chat opened
/// again or an answer is still coming.
fn unload_local_model_later(app: &AppHandle) {
    if local_ollama(app).is_none() {
        return;
    }
    let opened = app.state::<ChatState>().opened.load(Ordering::SeqCst);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(UNLOAD_AFTER).await;
        while busy(&app) {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        if app.state::<ChatState>().opened.load(Ordering::SeqCst) != opened {
            return;
        }
        let Some((url, model)) = local_ollama(&app) else {
            return;
        };
        let _ = tauri::async_runtime::spawn_blocking(move || {
            if let Some(ollama) = exile_assistant::Ollama::detect(&url, &model, USER_AGENT) {
                match ollama.unload() {
                    Ok(()) => log::info!("unloaded {model}"),
                    Err(e) => log::info!("couldn't unload {model}: {e}"),
                }
            }
        })
        .await;
    });
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
    unload_local_model_later(app);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_answer_reads_like_the_window() {
        let mut answer = String::new();
        for e in [
            ChatEvent::Paragraph,
            ChatEvent::Tool {
                name: "what_next".into(),
            },
            ChatEvent::Text {
                text: "Checking.".into(),
            },
            ChatEvent::Paragraph,
            ChatEvent::Paragraph,
            ChatEvent::Note {
                text: "Measuring".into(),
            },
            ChatEvent::Text {
                text: "Help Alira.".into(),
            },
        ] {
            add_to_answer(&mut answer, &e);
        }
        assert_eq!(answer, "Checking.\n\nHelp Alira.");
    }

    #[test]
    fn the_chat_scales_with_the_monitor_like_the_game() {
        assert_eq!(screen_zoom(1080.0), 1.0);
        assert!((screen_zoom(1440.0) - 4.0 / 3.0).abs() < 1e-9);
        assert_eq!(screen_zoom(2160.0), 2.0);
        assert_eq!(screen_zoom(720.0), 0.75);
    }

    #[test]
    fn providers_map_to_their_message_shape() {
        assert_eq!(dialect_of(Provider::Anthropic), Dialect::Anthropic);
        assert_eq!(dialect_of(Provider::OpenRouter), Dialect::OpenAi);
        assert_eq!(dialect_of(Provider::Custom), Dialect::OpenAi);
    }
}
