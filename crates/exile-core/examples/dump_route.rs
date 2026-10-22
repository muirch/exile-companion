//! Writes the first N route views as JSON, for the browser mock:
//! `cargo run -p exile-core --example dump_route -- 60 > src/ipc/mock-route.json`
//! The views carry the gem plan for a small Witch build, matching the mock's imported build.

use std::collections::HashSet;

use exile_core::data::DataPack;
use exile_core::gemplan::{annotate, plan, GemHints};
use exile_core::gems::GemTable;
use exile_core::route::parse_route;
use exile_pob::{Gem, PobBuild, SkillGroup, SkillSet};

fn mock_build() -> PobBuild {
    let gem = |id: &str, name: &str, support: bool| Gem {
        name_spec: Some(name.to_string()),
        gem_id: Some(format!("Metadata/Items/Gems/{id}")),
        enabled: true,
        is_support: support,
        ..Gem::default()
    };
    let group = |gems| SkillGroup {
        enabled: true,
        gems,
        ..SkillGroup::default()
    };
    PobBuild {
        class_name: Some("Witch".into()),
        skill_sets: vec![SkillSet {
            title: Some("Act 1".into()),
            groups: vec![
                group(vec![
                    gem("SkillGemRaiseZombie", "Raise Zombie", false),
                    gem("SupportGemArcaneSurge", "Arcane Surge Support", true),
                ]),
                group(vec![
                    gem("SkillGemSummonSkeletons", "Summon Skeletons", false),
                    gem("SupportGemMinionDamage", "Minion Damage Support", true),
                ]),
            ],
            ..SkillSet::default()
        }],
        ..PobBuild::default()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let count: usize = std::env::args().nth(1).map_or(Ok(14), |n| n.parse())?;
    let pack = DataPack::from_bytes(
        exile_data::AREAS_JSON,
        exile_data::AREAS_SCHEMA_VERSION,
        exile_data::QUESTS_JSON,
        exile_data::QUESTS_SCHEMA_VERSION,
        exile_data::TRIALS_JSON,
        exile_data::TRIALS_SCHEMA_VERSION,
    )?;
    let defines = HashSet::from(["LEAGUE_START".to_string()]);
    let source = exile_data::ROUTE_ACTS.join("\n");
    let (route, _) = parse_route(&source, &pack.zones, &pack.quests, &defines);
    let gems = GemTable::from_json_bytes(exile_data::GEMS_JSON, exile_data::GEM_SOURCES_JSON)?;
    let gem_plan = plan(&mock_build(), "Witch", &gems);
    // The mock also needs the zone each view belongs to, to fill the header.
    let views: Vec<_> = (0..count)
        .filter_map(|e| route.view(e))
        .map(|mut view| {
            annotate(&mut view, &gem_plan, GemHints::RewardsAndVendors);
            let zone = pack.zones.by_id(&view.area_id);
            serde_json::json!({
                "view": view,
                "zone": zone.map(|z| z.name.clone()),
                "act": zone.map(|z| z.act),
                "town": zone.is_some_and(|z| z.is_town),
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&views)?);
    Ok(())
}
