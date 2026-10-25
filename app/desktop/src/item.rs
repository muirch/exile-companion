//! The item check key: copies the item under the cursor and shows what it means for the build in
//! the overlay.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
#[cfg(windows)]
use std::time::Duration;

use exile_assistant::{GearCard, Tools};
use exile_core::item_check::{self, CopiedItem, GemCard};
use exile_core::level_up::{self, Remembered, UsableNow};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ItemCard {
    Measuring {
        name: String,
    },
    Gem(GemCard),
    Gear {
        name: String,
        card: GearCard,
        /// The character's level is below the item's, so it is remembered for the level-up.
        needs_level: Option<u32>,
    },
    /// A level-up made gems of the build or remembered upgrades usable.
    Ready(UsableNow),
    /// The player entered Kitava's zone and some resistance falls below the cap after him.
    Kitava {
        penalty: i32,
        short: Vec<exile_assistant::kitava::Short>,
    },
    /// Why there is nothing to show, in the player's words.
    Missing {
        name: String,
        message: String,
    },
}

fn emit(app: &AppHandle, card: &ItemCard) {
    let _ = app.emit("item://card", card);
}

static CHECKING: AtomicBool = AtomicBool::new(false);

/// Upgrades checked before the character could wear them. Kept until they come due or the app
/// quits.
static REMEMBERED: Mutex<Vec<Remembered>> = Mutex::new(Vec::new());

/// Called on entering Kitava's zone. Measures without a model and says nothing when the
/// resistances stay capped or there's no build.
pub fn kitava(app: &AppHandle, penalty: i32) {
    use exile_assistant::kitava;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let has_build = {
            let state = app.state::<AppState>();
            let builds = state.build.lock().unwrap_or_else(|e| e.into_inner());
            builds.build().is_some()
        };
        if !has_build {
            return;
        }
        let Ok(server) = crate::chat::server(&app).await else {
            return;
        };
        let read = |out: exile_assistant::ToolOutput| {
            (!out.is_error)
                .then(|| serde_json::from_str::<serde_json::Value>(&out.text).ok())
                .flatten()
        };
        let defences = read(server.call("defences", serde_json::json!({})).await);
        let what_if = read(
            server
                .call("what_if", kitava::penalty_change(penalty))
                .await,
        );
        let (Some(defences), Some(what_if)) = (defences, what_if) else {
            log::warn!("kitava check: the tools didn't answer");
            return;
        };
        let short = kitava::short_resists(&defences, &what_if);
        if !short.is_empty() {
            emit(&app, &ItemCard::Kitava { penalty, short });
        }
    });
}

/// Called when the log says the character went from level `from` to `to`.
pub fn level_up(app: &AppHandle, from: u32, to: u32) {
    let plan = crate::runtime::current_gem_plan(app);
    let mut list = REMEMBERED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(now) = level_up::usable_now(plan.as_ref(), &mut list, from, to) {
        emit(app, &ItemCard::Ready(now));
    }
}

/// The game puts the text on the clipboard within a few frames.
#[cfg(windows)]
const POLL: Duration = Duration::from_millis(48);
#[cfg(windows)]
const POLL_LIMIT: Duration = Duration::from_millis(500);

pub fn check(app: &AppHandle) {
    if CHECKING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(text) = copy_item_text(&app).await {
            if let Some(item) = item_check::parse(&text) {
                show(&app, &item, text).await;
            }
        }
        CHECKING.store(false, Ordering::SeqCst);
    });
}

/// Ctrl+C in the game, then wait for the item text. An item already on the clipboard is cleared
/// first, so an old copy isn't taken for the new one.
async fn copy_item_text(app: &AppHandle) -> Option<String> {
    #[cfg(windows)]
    {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        let clipboard = app.clipboard();
        let before = clipboard.read_text().ok().unwrap_or_default();
        let had_item = item_check::parse(&before).is_some();
        if had_item {
            let _ = clipboard.write_text("");
        }
        crate::hotkeys::copy_item();
        let mut waited = Duration::ZERO;
        let mut found = None;
        while waited < POLL_LIMIT {
            tokio::time::sleep(POLL).await;
            waited += POLL;
            let now = clipboard.read_text().ok().unwrap_or_default();
            if item_check::parse(&now).is_some() {
                found = Some(now);
                break;
            }
        }
        if !had_item {
            let _ = clipboard.write_text(before);
        }
        found
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        None
    }
}

async fn show(app: &AppHandle, item: &CopiedItem, text: String) {
    let name = item.title();
    let missing = |message: &str| ItemCard::Missing {
        name: name.clone(),
        message: message.into(),
    };
    let has_build = {
        let state = app.state::<AppState>();
        let builds = state.build.lock().unwrap_or_else(|e| e.into_inner());
        builds.build().is_some()
    };
    if !has_build {
        emit(
            app,
            &missing("Import your build in the settings to check items against it."),
        );
        return;
    }
    if item.is_gem() {
        let plan = crate::runtime::current_gem_plan(app);
        let card = {
            let state = app.state::<AppState>();
            let builds = state.build.lock().unwrap_or_else(|e| e.into_inner());
            builds
                .build()
                .map(|b| item_check::gem_card(b, plan.as_ref(), &item.name))
        };
        if let Some(card) = card {
            emit(app, &ItemCard::Gem(card));
        }
        return;
    }
    emit(app, &ItemCard::Measuring { name: name.clone() });
    let server = match crate::chat::server(app).await {
        Ok(s) => s,
        Err(e) => {
            log::warn!("item check: {e}");
            emit(
                app,
                &missing("Path of Building isn't running for the assistant."),
            );
            return;
        }
    };
    let out = server
        .call("compare_item", serde_json::json!({ "text": text }))
        .await;
    if out.is_error {
        log::warn!("compare_item: {}", out.text);
        emit(app, &missing(&out.text));
        return;
    }
    let card = serde_json::from_str::<serde_json::Value>(&out.text)
        .ok()
        .and_then(|v| exile_assistant::from_compare_item(&v));
    let level = app.state::<AppState>().current_state().level;
    match card {
        Some(card) => {
            let needs_level = item.required_level.filter(|&l| l > level && level > 0);
            if let Some(l) = needs_level {
                if matches!(
                    card.verdict,
                    exile_assistant::Verdict::Upgrade | exile_assistant::Verdict::Mixed
                ) {
                    level_up::remember(
                        &mut REMEMBERED.lock().unwrap_or_else(|e| e.into_inner()),
                        Remembered {
                            name: name.clone(),
                            slot: card.slot.clone(),
                            level: l,
                        },
                    );
                }
            }
            emit(
                app,
                &ItemCard::Gear {
                    name: name.clone(),
                    card,
                    needs_level,
                },
            );
        }
        None => emit(app, &missing("It doesn't go in any slot of your build.")),
    }
}
