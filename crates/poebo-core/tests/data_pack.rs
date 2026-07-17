//! Validates that the *committed* data pack (`data-pack/areas.json`, embedded via `poebo-data`)
//! parses at the schema version this build expects and contains known campaign anchors. This is the
//! guard that catches data-pack schema drift after a `tools/data-extractor` regen.

use poebo_core::data::ZoneTable;

#[test]
fn committed_pack_parses_and_has_act1_anchors() {
    let table = ZoneTable::from_json_bytes(poebo_data::AREAS_JSON, poebo_data::PACK_SCHEMA_VERSION)
        .expect("committed data-pack/areas.json must parse at the expected schema version");

    assert!(!table.is_empty(), "data pack has no areas");

    let strand = table
        .by_name("The Twilight Strand")
        .expect("The Twilight Strand must be present");
    assert_eq!(strand.act, 1, "Twilight Strand is in act 1");

    let watch = table
        .by_name("Lioneye's Watch")
        .expect("Lioneye's Watch must be present");
    assert_eq!(watch.act, 1);
    assert!(watch.is_town, "Lioneye's Watch is a town");
    assert!(watch.has_waypoint, "Lioneye's Watch has a waypoint");
}
