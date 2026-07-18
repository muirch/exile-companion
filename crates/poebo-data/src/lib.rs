//! Committed, versioned PoE campaign data pack, embedded at compile time.
//!
//! This crate deliberately carries only **bytes + version metadata**. Parsing lives in
//! [`poebo_core::data`], so the pure-logic crate never depends on this one. The Tauri shell wires
//! them together, handing these bytes to `poebo_core::data::DataPack::from_bytes`.
//!
//! The embedded JSON is generated from Path of Exile's game files by `tools/data-extractor` and
//! committed under `data-pack/`. The app build never runs the extractor. Each file carries its own
//! schema version so the shape of one pack can change without touching the others.

/// Schema version expected for `areas.json`. Must equal its `schema_version` field.
pub const AREAS_SCHEMA_VERSION: u32 = 1;
/// Schema version expected for `quests.json`.
pub const QUESTS_SCHEMA_VERSION: u32 = 1;
/// Schema version expected for `trials.json`.
pub const TRIALS_SCHEMA_VERSION: u32 = 1;

/// Raw bytes of the committed areas pack (`data-pack/areas.json`).
pub const AREAS_JSON: &[u8] = include_bytes!("../../../data-pack/areas.json");
/// Raw bytes of the committed quest/gem-reward pack (`data-pack/quests.json`).
pub const QUESTS_JSON: &[u8] = include_bytes!("../../../data-pack/quests.json");
/// Raw bytes of the committed lab-trials pack (`data-pack/trials.json`).
pub const TRIALS_JSON: &[u8] = include_bytes!("../../../data-pack/trials.json");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_packs_are_non_empty_json_objects() {
        for pack in [AREAS_JSON, QUESTS_JSON, TRIALS_JSON] {
            assert!(!pack.is_empty());
            // Cheap sanity check without pulling in a JSON parser dependency.
            assert_eq!(pack.iter().find(|b| !b.is_ascii_whitespace()), Some(&b'{'));
        }
    }
}
