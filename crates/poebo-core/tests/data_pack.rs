//! Validates that the *committed* data pack (`data-pack/{areas,quests,trials}.json`, embedded via
//! `poebo-data`) parses at the schema versions this build expects and contains known campaign
//! anchors. This is the guard that catches data-pack schema drift after a `tools/data-extractor`
//! regen.

use poebo_core::data::{DataPack, ZoneTable};

fn committed_pack() -> DataPack {
    DataPack::from_bytes(
        poebo_data::AREAS_JSON,
        poebo_data::AREAS_SCHEMA_VERSION,
        poebo_data::QUESTS_JSON,
        poebo_data::QUESTS_SCHEMA_VERSION,
        poebo_data::TRIALS_JSON,
        poebo_data::TRIALS_SCHEMA_VERSION,
    )
    .expect("committed data pack must parse at the expected schema versions")
}

#[test]
fn committed_pack_parses_and_has_act1_anchors() {
    let table =
        ZoneTable::from_json_bytes(poebo_data::AREAS_JSON, poebo_data::AREAS_SCHEMA_VERSION)
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
    let table =
        ZoneTable::from_json_bytes(poebo_data::AREAS_JSON, poebo_data::AREAS_SCHEMA_VERSION)
            .expect("parse committed pack");
    let watch = table.zones_named("Lioneye's Watch");
    assert!(
        watch.len() >= 2,
        "Lioneye's Watch should appear in multiple acts (1 and 6), got {}",
        watch.len()
    );
}

#[test]
fn committed_quests_have_act1_gem_rewards() {
    let pack = committed_pack();
    assert!(!pack.quests.is_empty(), "quests pack is empty");
    // "Enemy at the Gate" (a1q1) rewards a starter skill gem to the Witch.
    let a1q1 = pack
        .quests
        .all()
        .iter()
        .find(|q| q.id == "a1q1")
        .expect("a1q1 present");
    assert!(
        a1q1.gem_rewards.iter().any(|r| r.allows_class("Witch")),
        "a1q1 should reward a Witch-claimable gem"
    );
}

#[test]
fn committed_trials_cover_the_normal_lab() {
    let pack = committed_pack();
    // 6 Normal Labyrinth trials live in acts 1-3; 12 campaign trials in total.
    assert_eq!(pack.trials.all().len(), 12, "expected 12 campaign lab trials");
    let acts_1_3 = pack.trials.all().iter().filter(|t| t.act <= 3).count();
    assert_eq!(acts_1_3, 6, "expected 6 Normal Labyrinth trials in acts 1-3");
    assert!(
        pack.trials.in_act(1).any(|t| t.area_name == "The Lower Prison"),
        "The Lower Prison is the Act 1 trial"
    );
}
