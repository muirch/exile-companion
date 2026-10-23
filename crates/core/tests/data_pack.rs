//! Validates that the *committed* data pack (`crates/data/data/{areas,quests,trials}.json`, embedded via
//! `exile-data`) parses at the schema versions this build expects and contains known campaign
//! anchors. This is the guard that catches data-pack schema drift after a `tools/data-extractor`
//! regen.

use exile_core::data::{DataPack, ZoneTable};

fn committed_pack() -> DataPack {
    DataPack::from_bytes(
        exile_data::AREAS_JSON,
        exile_data::AREAS_SCHEMA_VERSION,
        exile_data::QUESTS_JSON,
        exile_data::QUESTS_SCHEMA_VERSION,
        exile_data::TRIALS_JSON,
        exile_data::TRIALS_SCHEMA_VERSION,
    )
    .expect("committed data pack must parse at the expected schema versions")
}

#[test]
fn committed_pack_parses_and_has_act1_anchors() {
    let table =
        ZoneTable::from_json_bytes(exile_data::AREAS_JSON, exile_data::AREAS_SCHEMA_VERSION)
            .expect(
                "committed crates/data/data/areas.json must parse at the expected schema version",
            );

    assert!(!table.is_empty(), "data pack has no areas");

    // The Twilight Strand is the Act 1 starting zone (id 1_1_1). An Act 6 copy also exists.
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
        ZoneTable::from_json_bytes(exile_data::AREAS_JSON, exile_data::AREAS_SCHEMA_VERSION)
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
    // "Enemy at the Gate" (a1q1) rewards a starter skill gem to the Witch, claimed from Tarkleigh.
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
    assert_eq!(a1q1.npc.as_deref(), Some("Tarkleigh"), "a1q1 reward NPC");
    // Exactly 12 standard gem-reward quests (Tencent-only offers must be excluded).
    let gem_quests = pack
        .quests
        .all()
        .iter()
        .filter(|q| !q.gem_rewards.is_empty())
        .count();
    assert_eq!(gem_quests, 12, "expected 12 standard gem-reward quests");
}

#[test]
fn committed_quests_grant_the_expected_passive_points() {
    let pack = committed_pack();
    // 22 passive skill points come from "Book of Skill" quest rewards across the campaign.
    let total: u32 = pack.quests.all().iter().map(|q| q.passive_points).sum();
    assert_eq!(total, 22, "expected 22 quest passive points");
    // "The Way Forward" (a1q9) grants a point and no gem.
    let a1q9 = pack
        .quests
        .all()
        .iter()
        .find(|q| q.id == "a1q9")
        .expect("a1q9");
    assert_eq!(a1q9.passive_points, 1);
    assert!(a1q9.gem_rewards.is_empty());
    assert!(pack.quests.passive_point_quests_in_act(1).count() >= 1);
}

#[test]
fn committed_trials_cover_the_normal_lab() {
    let pack = committed_pack();
    // 6 Normal Labyrinth trials live in acts 1-3. 12 campaign trials in total.
    assert_eq!(
        pack.trials.all().len(),
        12,
        "expected 12 campaign lab trials"
    );
    let acts_1_3 = pack.trials.all().iter().filter(|t| t.act <= 3).count();
    assert_eq!(
        acts_1_3, 6,
        "expected 6 Normal Labyrinth trials in acts 1-3"
    );
    assert!(
        pack.trials
            .in_act(1)
            .any(|t| t.area_name == "The Lower Prison"),
        "The Lower Prison is the Act 1 trial"
    );
}

#[test]
fn bundled_route_parses_cleanly() {
    use exile_core::route::parse_route;
    use std::collections::HashSet;

    let pack = committed_pack();
    let source = exile_data::ROUTE_ACTS.join("\n");
    for defines in [HashSet::new(), HashSet::from(["LEAGUE_START".to_string()])] {
        let (route, warnings) = parse_route(&source, &pack.zones, &pack.quests, &defines);
        assert!(warnings.is_empty(), "{warnings:#?}");
        assert_eq!(route.sections.len(), 10);
        assert_eq!(route.edges.first().map(String::as_str), Some("1_1_1"));
        assert!(route.edges.len() > 100, "only {} edges", route.edges.len());
    }
}

#[test]
fn tracker_walks_the_bundled_route_end_to_end() {
    use exile_core::route::{parse_route, RouteTracker};
    use std::collections::HashSet;

    let pack = committed_pack();
    let source = exile_data::ROUTE_ACTS.join("\n");
    let (route, _) = parse_route(&source, &pack.zones, &pack.quests, &HashSet::new());
    let mut tracker = RouteTracker::default();
    for (i, area) in route.edges.iter().enumerate().skip(1) {
        assert!(
            route.view(tracker.active()).is_some(),
            "no view for edge {i}"
        );
        assert!(
            tracker.on_area_generated(&route, area),
            "stuck before edge {i} ({area})"
        );
    }
    assert_eq!(tracker.active(), route.edges.len() - 1);
}

#[test]
fn high_level_character_in_act1_town_gets_the_last_act1_visit() {
    use exile_core::route::{parse_route, RouteTracker};
    use std::collections::HashSet;

    let pack = committed_pack();
    let source = exile_data::ROUTE_ACTS.join("\n");
    let (route, _) = parse_route(&source, &pack.zones, &pack.quests, &HashSet::new());
    let town_visits: Vec<usize> = (0..route.edges.len())
        .filter(|&i| route.edges[i] == "1_1_town")
        .collect();

    let mut fresh = RouteTracker::default();
    fresh.on_area(&route, "1_1_town", Some(2));
    assert_eq!(fresh.active(), town_visits[0]);

    let mut done = RouteTracker::default();
    done.on_area(&route, "1_1_town", Some(90));
    assert_eq!(Some(&done.active()), town_visits.last());
}
