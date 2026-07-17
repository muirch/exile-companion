//! Committed, versioned PoE campaign data pack, embedded at compile time.
//!
//! This crate deliberately carries only **bytes + version metadata**. Parsing lives in
//! [`poebo_core::data`], so the pure-logic crate never depends on this one. The Tauri shell wires
//! them together: it hands [`AREAS_JSON`] to `poebo_core::data::ZoneTable::from_json_bytes`.
//!
//! The embedded JSON is generated from Path of Exile's game files by `tools/data-extractor` and
//! committed under `data-pack/`. The app build never runs the extractor.

/// Data-pack schema version this build expects. Must equal the `schema_version` field inside
/// [`AREAS_JSON`]; the loader rejects a mismatch.
pub const PACK_SCHEMA_VERSION: u32 = 1;

/// Raw bytes of the committed areas data pack (`data-pack/areas.json`).
pub const AREAS_JSON: &[u8] = include_bytes!("../../../data-pack/areas.json");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_pack_is_non_empty_json_object() {
        assert!(!AREAS_JSON.is_empty());
        // Cheap sanity check without pulling in a JSON parser dependency.
        assert_eq!(
            AREAS_JSON.iter().find(|b| !b.is_ascii_whitespace()),
            Some(&b'{')
        );
    }
}
