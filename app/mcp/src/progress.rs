//! Where the player is in the game, from Client.txt, the log the game writes anyway.

use std::path::Path;
use std::sync::Arc;

use exile_core::data::ZoneTable;
use exile_core::locale::LocalePack;
use serde_json::{json, Value};

pub fn game_progress(log: Option<&Path>) -> Value {
    let found = match log {
        Some(log) => Some(log.to_path_buf()),
        None => exile_core::logfile::find_client_log(crate::paths::saved_log().as_deref()),
    };
    let Some(log) = found else {
        return json!({ "missing": "Client.txt wasn't found. The player can pick it in Exile Companion's settings." });
    };
    let zones = match ZoneTable::from_json_bytes(
        exile_data::AREAS_JSON,
        exile_data::AREAS_SCHEMA_VERSION,
    ) {
        Ok(z) => z,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    let state = match exile_core::pipeline::state_from_log(&log, Arc::new(zones), &LocalePack::EN) {
        Ok(s) => s,
        Err(e) => return json!({ "error": format!("{}: {e}", log.display()) }),
    };
    let minutes_ago = std::fs::metadata(&log)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map(|d| d.as_secs() / 60);
    json!({
        "character": state.character,
        "class": state.class,
        // The log names the level only when it goes up.
        "level": (state.level > 0).then_some(state.level),
        "act": state.act,
        "zone": state.zone.map(|z| z.name),
        "area_id": state.area_id,
        "location": state.location_kind,
        "deaths": state.deaths,
        "log_updated_minutes_ago": minutes_ago,
    })
}
