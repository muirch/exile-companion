//! Validates that the *committed* data pack (`data-pack/areas.json`, embedded via `poebo-data`)
//! parses at the schema version this build expects and contains known campaign anchors. This is the
//! guard that catches data-pack schema drift after a `tools/data-extractor` regen.

use poebo_core::data::ZoneTable;

#[test]
fn committed_pack_parses_and_has_act1_anchors() {
    let table = ZoneTable::from_json_bytes(poebo_data::AREAS_JSON, poebo_data::PACK_SCHEMA_VERSION)
        .expect("committed data-pack/areas.json must parse at the expected schema version");

    assert!(!table.is_empty(), "data pack has no areas");

    // The Twilight Strand is the Act 1 starting zone (id 1_1_1); an Act 6 copy also exists.
    let strand = table.zones_named("The Twilight Strand");
    assert!(
        strand.iter().any(|z| z.act == 1 && z.id == "1_1_1"),
        "expected an Act 1 'The Twilight Strand' (1_1_1)"
    );

    // Lioneye's Watch is the Act 1 town with a waypoint (and repeats in Act 6).
    let watch = table.zones_named("Lioneye's Watch");
    let act1_watch = watch
        .iter()
        .find(|z| z.act == 1)
        .expect("expected an Act 1 'Lioneye's Watch'");
    assert!(act1_watch.is_town, "Act 1 Lioneye's Watch is a town");
    assert!(
        act1_watch.has_waypoint,
        "Act 1 Lioneye's Watch has a waypoint"
    );
}

#[test]
fn committed_pack_has_cross_act_name_collisions() {
    // The key real-world fact the state machine must handle: names repeat across acts.
    let table = ZoneTable::from_json_bytes(poebo_data::AREAS_JSON, poebo_data::PACK_SCHEMA_VERSION)
        .expect("parse committed pack");
    let watch = table.zones_named("Lioneye's Watch");
    assert!(
        watch.len() >= 2,
        "Lioneye's Watch should appear in multiple acts (1 and 6), got {}",
        watch.len()
    );
}
