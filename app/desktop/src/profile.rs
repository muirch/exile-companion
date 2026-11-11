//! The guide the player follows and their own character, both through the assistant's tools so the
//! app and the chat share one way of doing it. The build and guide files change on disk, and
//! watch_build picks that up.

use exile_assistant::Tools;
use serde_json::{json, Value};
use tauri::AppHandle;

async fn tool(app: &AppHandle, name: &str, input: Value) -> Result<Value, String> {
    let server = crate::chat::server(app)
        .await
        .map_err(|e| e.player_message(exile_assistant::Provider::Anthropic))?;
    let out = server.call(name, input).await;
    if out.is_error {
        log::warn!("{name}: {}", out.text);
        return Err(exile_assistant::engine_error::for_player(&out.text));
    }
    serde_json::from_str(&out.text)
        .map_err(|e| format!("The tools answered something unreadable ({e})."))
}

/// Picks the change up at once instead of on the next poll.
fn refresh(app: &AppHandle) {
    let before = crate::runtime::plan_source(app);
    crate::runtime::restore_build(app);
    crate::runtime::restore_guide(app);
    crate::runtime::keep_or_drop_stages(app, before.as_ref());
    crate::runtime::emit_build_summary(app);
    crate::runtime::emit_route(app);
}

pub async fn follow_guide(app: &AppHandle, text: String) -> Result<(), String> {
    tool(app, "follow_guide", json!({ "guide": text })).await?;
    refresh(app);
    Ok(())
}

pub async fn forget_guide(app: &AppHandle) -> Result<(), String> {
    tool(app, "forget_guide", json!({})).await?;
    refresh(app);
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterList {
    pub account: String,
    pub characters: Vec<CharacterEntry>,
    /// The one the game's log says is played.
    pub playing: Option<String>,
}

// Defaults, so one odd character in the account's answer doesn't make the whole list unreadable.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CharacterEntry {
    pub name: String,
    pub class: String,
    pub level: u32,
    pub league: String,
}

pub async fn list_characters(app: &AppHandle, account: String) -> Result<CharacterList, String> {
    let v = tool(app, "list_characters", json!({ "account": account })).await?;
    let characters = serde_json::from_value(v["characters"].clone())
        .map_err(|e| format!("The character list was unreadable ({e})."))?;
    Ok(CharacterList {
        account: v["account"].as_str().unwrap_or(&account).to_string(),
        characters,
        playing: v["playing"].as_str().map(str::to_string),
    })
}

/// Imports the character as the player's build. With no name, the last imported one again.
pub async fn import_character(
    app: &AppHandle,
    account: Option<String>,
    character: Option<String>,
) -> Result<(), String> {
    tool(
        app,
        "import_character",
        json!({ "account": account, "character": character }),
    )
    .await?;
    refresh(app);
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub guide: Option<crate::runtime::GuideView>,
    pub character: Option<exile_core::build::BuildSummary>,
    /// Where the character came from, when it was imported from pathofexile.com.
    pub link: Option<exile_core::character::CharacterLink>,
}

pub fn profile(app: &AppHandle) -> Profile {
    Profile {
        guide: crate::runtime::current_guide(app),
        character: crate::runtime::current_character_summary(app),
        link: crate::runtime::current_character(app),
    }
}
