//! The guidance engine: fold `CampaignState + BuildState + DataPack` into a single [`NextObjective`]
//! for the overlay's "Next objective" line.
//!
//! This is deliberately a small, fixed-priority set of high-confidence hints (it does not yet track
//! which quests are done, so it surfaces what is *relevant* to the current act/zone rather than a
//! precise checklist):
//!
//! 1. Gems from the build to claim in the current act (needs a build).
//! 2. When out in a zone: the next area to head to (via zone connectivity).
//! 3. The current act's Trial of Ascendancy.
//! 4. A fallback build/spec summary, else an idle prompt.

use crate::build::BuildState;
use crate::data::{DataPack, ZoneInfo};
use crate::resolve::{GemResolver, TreeResolver};
use crate::state::{CampaignState, LocationKind};

/// The kind of objective, so the UI can style it. Serialized in snake_case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveKind {
    /// A gem the build uses is claimable from a quest now.
    QuestReward,
    /// Head to the next campaign zone.
    ProceedToZone,
    /// A Trial of Ascendancy is in this act.
    LabTrial,
    /// General build/spec context (nothing more specific applies).
    BuildSpec,
    /// Nothing to suggest yet.
    Idle,
}

/// The single next-objective hint rendered by the overlay. Emitted as `guidance://next`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
pub struct NextObjective {
    pub kind: ObjectiveKind,
    /// The primary one-line instruction.
    pub headline: String,
    /// Optional supporting detail (quest name, act, main skill, ...).
    pub detail: Option<String>,
}

impl NextObjective {
    fn new(kind: ObjectiveKind, headline: impl Into<String>, detail: Option<String>) -> Self {
        Self {
            kind,
            headline: headline.into(),
            detail,
        }
    }
}

pub struct GuidanceEngine;

impl GuidanceEngine {
    /// Compute the next objective from the current state, loaded build, and data pack.
    pub fn next(state: &CampaignState, build_state: &BuildState, data: &DataPack) -> NextObjective {
        let build = build_state.build();

        // 1. Build gems claimable in the current act (most actionable).
        if let Some(build) = build {
            let picks = GemResolver::available_now(state, data, build);
            if let Some(first) = picks.first() {
                // Quest gems are claimed from the quest-reward NPC, not looted, so "Claim".
                let headline = if picks.len() == 1 {
                    format!("Claim {}", first.gem_name)
                } else {
                    format!("Claim {} (+{} more)", first.gem_name, picks.len() - 1)
                };
                let detail = Some(format!(
                    "Quest reward from {} (Act {})",
                    first.quest_name, first.act
                ));
                return NextObjective::new(ObjectiveKind::QuestReward, headline, detail);
            }
        }

        // 2. When actively in a zone (not town/hideout), point to the next area.
        let in_town = matches!(
            state.location_kind,
            LocationKind::Town | LocationKind::Hideout
        );
        if !in_town {
            if let Some(obj) = proceed_objective(state, data) {
                return obj;
            }
        }

        // 3. The current act's lab trial.
        if let Some(act) = state.act {
            if let Some(trial) = data.trials.in_act(act).next() {
                return NextObjective::new(
                    ObjectiveKind::LabTrial,
                    format!("Trial of Ascendancy: {}", trial.area_name),
                    Some(format!("Act {} lab trial", act)),
                );
            }
        }

        // 4. Fall back to proceeding (even from town), then a build summary, then idle.
        if let Some(obj) = proceed_objective(state, data) {
            return obj;
        }
        if let Some(build) = build {
            if let Some(resolved) = TreeResolver::resolve(state, build) {
                let headline = match resolved.spec.title.as_deref() {
                    Some(title) => format!("Build tree: {}", title),
                    None => "Following your imported build".to_string(),
                };
                let detail = build
                    .main_group()
                    .and_then(|g| g.first_active_gem())
                    .and_then(|gem| gem.name_spec.clone())
                    .map(|skill| format!("Main skill: {}", skill));
                return NextObjective::new(ObjectiveKind::BuildSpec, headline, detail);
            }
        }

        idle_objective(state, build.is_some())
    }
}

/// Suggest the next area to move to from the current zone, using campaign connectivity: the
/// lowest-level non-town connection whose area level is above the current zone's (a step forward).
fn proceed_objective(state: &CampaignState, data: &DataPack) -> Option<NextObjective> {
    let zone = state.zone.as_ref()?;
    let id = zone.id.as_ref()?;
    let here = data.zones.by_id(id)?;

    let next: Option<&ZoneInfo> = here
        .connections
        .iter()
        .filter_map(|c| data.zones.by_id(c))
        .filter(|z| !z.is_town && z.area_level > here.area_level)
        .min_by_key(|z| z.area_level);

    next.map(|z| {
        NextObjective::new(
            ObjectiveKind::ProceedToZone,
            format!("Head to {}", z.name),
            Some(format!("Act {}", z.act)),
        )
    })
}

fn idle_objective(state: &CampaignState, has_build: bool) -> NextObjective {
    if !has_build {
        return NextObjective::new(
            ObjectiveKind::Idle,
            "Import a Path of Building build",
            Some("Load a build (file or paste) for gem and tree guidance".to_string()),
        );
    }
    let headline = if state.session_started {
        "Keep progressing the campaign"
    } else {
        "Waiting for the game"
    };
    NextObjective::new(ObjectiveKind::Idle, headline, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ZoneRef;
    use poebo_pob::{Gem, PobBuild, SkillGroup, SkillSet};

    const AREAS: &str = r#"{
        "schema_version":1,"patch":"t","generated_at":"t","source":"t","areas":[
            {"id":"1_1_town","name":"Lioneye's Watch","act":1,"area_level":1,
             "is_town":true,"is_hideout":false,"is_map":false,"has_waypoint":true,"connections":["1_1_1"]},
            {"id":"1_1_1","name":"The Twilight Strand","act":1,"area_level":1,
             "is_town":false,"is_hideout":false,"is_map":false,"has_waypoint":false,"connections":["1_1_2"]},
            {"id":"1_1_2","name":"The Coast","act":1,"area_level":2,
             "is_town":false,"is_hideout":false,"is_map":false,"has_waypoint":true,"connections":["1_1_1","1_1_3"]},
            {"id":"1_1_3","name":"The Mud Flats","act":1,"area_level":3,
             "is_town":false,"is_hideout":false,"is_map":false,"has_waypoint":false,"connections":["1_1_2"]}
        ]
    }"#;
    const QUESTS: &str = r#"{
        "schema_version":1,"patch":"t","generated_at":"t","source":"t","quests":[
            {"id":"a1q1","act":1,"name":"Enemy at the Gate","gem_rewards":[
                {"gem_id":"g/Freezing","gem_name":"Freezing Pulse","reward_level":1,"classes":["Witch"]}
            ]}
        ]
    }"#;
    const TRIALS: &str = r#"{
        "schema_version":1,"patch":"t","generated_at":"t","source":"t","trials":[
            {"area_id":"1_1_7_1","area_name":"The Lower Prison","act":1}
        ]
    }"#;

    fn pack() -> DataPack {
        DataPack::from_bytes(
            AREAS.as_bytes(),
            1,
            QUESTS.as_bytes(),
            1,
            TRIALS.as_bytes(),
            1,
        )
        .unwrap()
    }

    fn witch_build_with(gem_ids: &[&str]) -> PobBuild {
        PobBuild {
            class_name: Some("Witch".into()),
            skill_sets: vec![SkillSet {
                groups: vec![SkillGroup {
                    enabled: true,
                    gems: gem_ids
                        .iter()
                        .map(|id| Gem {
                            gem_id: Some(id.to_string()),
                            name_spec: Some("Freezing Pulse".into()),
                            enabled: true,
                            ..Gem::default()
                        })
                        .collect(),
                    ..SkillGroup::default()
                }],
                ..SkillSet::default()
            }],
            ..PobBuild::default()
        }
    }

    fn state(zone_id: &str, name: &str, act: u8, loc: LocationKind, level: u32) -> CampaignState {
        CampaignState {
            level,
            act: Some(act),
            zone: Some(ZoneRef {
                name: name.into(),
                id: Some(zone_id.into()),
                act: Some(act),
            }),
            location_kind: loc,
            session_started: true,
            ..CampaignState::default()
        }
    }

    #[test]
    fn gem_pickup_takes_priority() {
        let mut bs = BuildState::new();
        bs.set(witch_build_with(&["g/Freezing"]));
        // In town in act 1, a build using Freezing Pulse -> claim it.
        let obj = GuidanceEngine::next(
            &state("1_1_town", "Lioneye's Watch", 1, LocationKind::Town, 2),
            &bs,
            &pack(),
        );
        assert_eq!(obj.kind, ObjectiveKind::QuestReward);
        assert!(obj.headline.contains("Freezing Pulse"), "{}", obj.headline);
    }

    #[test]
    fn proceeds_forward_when_in_a_zone() {
        let bs = BuildState::new();
        // On The Coast (level 2, combat) -> step forward to The Mud Flats (level 3), not back to town.
        let obj = GuidanceEngine::next(
            &state("1_1_2", "The Coast", 1, LocationKind::Combat, 3),
            &bs,
            &pack(),
        );
        assert_eq!(obj.kind, ObjectiveKind::ProceedToZone);
        assert_eq!(obj.headline, "Head to The Mud Flats");
    }

    #[test]
    fn lab_trial_when_in_town_with_no_gem() {
        let bs = BuildState::new(); // no build -> no gem picks
                                    // In town, no build gems, act 1 has a trial.
        let obj = GuidanceEngine::next(
            &state("1_1_town", "Lioneye's Watch", 1, LocationKind::Town, 2),
            &bs,
            &pack(),
        );
        assert_eq!(obj.kind, ObjectiveKind::LabTrial);
        assert!(obj.headline.contains("The Lower Prison"));
    }

    #[test]
    fn idle_prompts_to_import_a_build_when_nothing_applies() {
        let bs = BuildState::new();
        // Unknown zone (no id) in an act with no trial and no build.
        let mut st = CampaignState {
            act: Some(4),
            session_started: true,
            ..CampaignState::default()
        };
        st.location_kind = LocationKind::Combat;
        let obj = GuidanceEngine::next(&st, &bs, &pack());
        assert_eq!(obj.kind, ObjectiveKind::Idle);
        assert!(obj.headline.contains("Import"));
    }
}
