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

/// exile-leveling's gem list (MIT), keyed by the gem's base item id.
pub const GEMS_JSON: &[u8] = include_bytes!("../../../data-pack/routes/exile-leveling/gems.json");
/// exile-leveling's quest rewards and vendor gems per quest (MIT).
pub const GEM_SOURCES_JSON: &[u8] =
    include_bytes!("../../../data-pack/routes/exile-leveling/quests.json");

/// exile-leveling's route, one file per act in order (MIT, see `data-pack/routes/exile-leveling`).
pub const ROUTE_ACTS: [&str; 10] = [
    include_str!("../../../data-pack/routes/exile-leveling/act-1.txt"),
    include_str!("../../../data-pack/routes/exile-leveling/act-2.txt"),
    include_str!("../../../data-pack/routes/exile-leveling/act-3.txt"),
    include_str!("../../../data-pack/routes/exile-leveling/act-4.txt"),
    include_str!("../../../data-pack/routes/exile-leveling/act-5.txt"),
    include_str!("../../../data-pack/routes/exile-leveling/act-6.txt"),
    include_str!("../../../data-pack/routes/exile-leveling/act-7.txt"),
    include_str!("../../../data-pack/routes/exile-leveling/act-8.txt"),
    include_str!("../../../data-pack/routes/exile-leveling/act-9.txt"),
    include_str!("../../../data-pack/routes/exile-leveling/act-10.txt"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_packs_are_non_empty_json_objects() {
        for pack in [
            AREAS_JSON,
            QUESTS_JSON,
            TRIALS_JSON,
            GEMS_JSON,
            GEM_SOURCES_JSON,
        ] {
            assert!(!pack.is_empty());
            // Cheap sanity check without pulling in a JSON parser dependency.
            assert_eq!(pack.iter().find(|b| !b.is_ascii_whitespace()), Some(&b'{'));
        }
    }
}
