//! Turns a build's gems into a campaign plan: which gem to take as each quest reward, and what to
//! buy from vendors after which quest.
//!
//! Every gem gets the level from which the build needs it. Leveling builds keep one skill set per
//! stage ("Act 1", "lvl 28", or just in order), so a gem is needed from the first set it shows up
//! in. A build with a single final set has no stages. Then a gem is needed from its own required
//! level. A quest reward goes to the gem needed soonest, and a vendor buy waits until the act the
//! gem is needed in, so the first town doesn't list the whole campaign.

use std::collections::HashSet;

use exile_pob::PobBuild;

use crate::gems::{quest_number, GemSource, GemSourceKind, GemTable};
use crate::resolve::{act_expected_level, title_target_level};
use crate::route::{Part, RouteView, Step};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GemAction {
    Take,
    Buy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GemStep {
    pub gem_id: String,
    pub name: String,
    pub quest_id: String,
    pub act: u32,
    pub action: GemAction,
    pub npc: String,
}

/// Which gem steps the route shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GemHints {
    Off,
    Rewards,
    #[default]
    RewardsAndVendors,
}

impl GemHints {
    fn shows(self, action: GemAction) -> bool {
        match self {
            Self::Off => false,
            Self::Rewards => action == GemAction::Take,
            Self::RewardsAndVendors => true,
        }
    }
}

/// What the player says a skill set is for, overriding what its title and order suggest.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[serde(rename_all = "camelCase")]
pub enum PresetStage {
    #[default]
    Auto,
    /// From the start of this act.
    Act(u8),
    Endgame,
    /// Not part of the plan, like a separator or an alternative setup.
    Skip,
}

/// A skill set as the settings list it: what the planner makes of it on its own.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
pub struct PresetInfo {
    pub title: Option<String>,
    pub gem_count: u32,
    /// The level the planner gives the set by itself. `None` until a set names a level or act.
    pub auto_level: Option<u32>,
}

pub fn presets(build: &PobBuild) -> Vec<PresetInfo> {
    build
        .skill_sets
        .iter()
        .zip(set_levels(build, &[]))
        .map(|(set, auto_level)| PresetInfo {
            title: set.title.clone(),
            gem_count: set
                .groups
                .iter()
                .filter(|g| g.enabled && g.source.is_none())
                .flat_map(|g| g.gems.iter().filter(|g| g.enabled))
                .count() as u32,
            auto_level,
        })
        .collect()
}

/// Whether stages the player set for `old`'s skill sets still fit `new`. They are kept by
/// position, so a build with other skill sets needs them set again.
pub fn same_presets(old: &PobBuild, new: &PobBuild) -> bool {
    old.skill_sets
        .iter()
        .map(|s| &s.title)
        .eq(new.skill_sets.iter().map(|s| &s.title))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GemPlan {
    /// In campaign order.
    pub steps: Vec<GemStep>,
    /// Names of the build's gems the campaign won't give this class: no quest or vendor has them,
    /// or every reward that has them went to another gem.
    pub not_in_campaign: Vec<String>,
}

impl GemPlan {
    pub fn for_quest<'a>(&'a self, quest_id: &'a str) -> impl Iterator<Item = &'a GemStep> + 'a {
        self.steps.iter().filter(move |s| s.quest_id == quest_id)
    }
}

struct Wanted {
    id: String,
    name: String,
    is_support: bool,
    /// Character level from which the build uses the gem.
    need_level: u32,
}

/// The level a skill set stands for: the player's stage, else its title if it names one, else no
/// later than the sets before it. Sets are kept in document order, which leveling builds write in
/// campaign order. Skipped sets don't move the floor.
fn set_levels(build: &PobBuild, stages: &[PresetStage]) -> Vec<Option<u32>> {
    let mut floor = None;
    build
        .skill_sets
        .iter()
        .enumerate()
        .map(|(i, set)| {
            let own = match stages.get(i).copied().unwrap_or_default() {
                PresetStage::Auto => set.title.as_deref().and_then(title_target_level),
                PresetStage::Act(act) => Some(act_expected_level(act.clamp(1, 10))),
                PresetStage::Endgame => Some(100),
                PresetStage::Skip => return floor,
            };
            floor = own.or(floor);
            floor
        })
        .collect()
}

/// Enabled gems from enabled socket groups, first appearance wins. Item-granted skills are left
/// out: nobody hands those out. Sets with no enabled gem (separators) add nothing.
fn wanted_gems(build: &PobBuild, gems: &GemTable, stages: &[PresetStage]) -> Vec<Wanted> {
    let levels = set_levels(build, stages);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (i, (set, set_level)) in build.skill_sets.iter().zip(levels).enumerate() {
        if stages.get(i) == Some(&PresetStage::Skip) {
            continue;
        }
        for group in set
            .groups
            .iter()
            .filter(|g| g.enabled && g.source.is_none())
        {
            for gem in group.gems.iter().filter(|g| g.enabled) {
                let Some(id) = gem.gem_id.clone() else {
                    continue;
                };
                if !seen.insert(id.clone()) {
                    continue;
                }
                let required = gems.gem(&id).map_or(1, |g| g.required_level);
                out.push(Wanted {
                    name: gem.name_spec.clone().unwrap_or_else(|| id.clone()),
                    id,
                    is_support: gem.is_support,
                    need_level: set_level.unwrap_or(0).max(required),
                });
            }
        }
    }
    out
}

/// The act a character of `level` is usually in.
fn act_for_level(level: u32) -> u32 {
    (1..=10u8)
        .rev()
        .find(|a| act_expected_level(*a) <= level)
        .map_or(1, u32::from)
}

/// Campaign order of a quest id like "a3q11": act, then quest number.
fn quest_order(s: &GemSource) -> (u32, u32) {
    (s.act, quest_number(&s.quest_id))
}

pub fn plan(build: &PobBuild, class: &str, gems: &GemTable) -> GemPlan {
    plan_with_stages(build, class, gems, &[])
}

/// `stages` are per skill set, in document order. Missing ones count as auto.
pub fn plan_with_stages(
    build: &PobBuild,
    class: &str,
    gems: &GemTable,
    stages: &[PresetStage],
) -> GemPlan {
    let wanted = wanted_gems(build, gems, stages);
    let mut sources: Vec<(usize, &GemSource)> = Vec::new();
    let mut not_in_campaign = Vec::new();
    for (i, w) in wanted.iter().enumerate() {
        let before = sources.len();
        sources.extend(gems.sources_for(&w.id, class).map(|s| (i, s)));
        if sources.len() == before {
            not_in_campaign.push(w.name.clone());
        }
    }
    sources.sort_by_key(|(i, s)| (quest_order(s), *i));

    let first_vendor = |i: usize| {
        sources
            .iter()
            .filter(|(j, s)| *j == i && s.kind == GemSourceKind::Vendor)
            .map(|(_, s)| quest_order(s))
            .min()
    };

    let mut placed: HashSet<usize> = HashSet::new();
    let mut steps = Vec::new();

    // One reward per offer, to the gem needed soonest. A gem a vendor already sells by then isn't
    // worth the reward.
    let mut offers: Vec<((u32, u32), &str)> = sources
        .iter()
        .filter(|(_, s)| s.kind == GemSourceKind::QuestReward)
        .map(|(_, s)| (quest_order(s), s.offer_id.as_str()))
        .collect();
    offers.sort();
    offers.dedup();
    for (q, offer) in offers {
        let pick = sources
            .iter()
            .filter(|(i, s)| {
                s.kind == GemSourceKind::QuestReward
                    && quest_order(s) == q
                    && s.offer_id == offer
                    && !placed.contains(i)
                    && first_vendor(*i).is_none_or(|v| v >= q)
            })
            .min_by_key(|(i, _)| (wanted[*i].need_level, wanted[*i].is_support, *i));
        if let Some((i, s)) = pick {
            placed.insert(*i);
            steps.push(step(&wanted[*i], s, GemAction::Take));
        }
    }

    // Buy at the first vendor in or after the act the gem is needed in, else the last one before.
    for (i, w) in wanted.iter().enumerate() {
        if placed.contains(&i) {
            continue;
        }
        let need_act = act_for_level(w.need_level);
        let vendors: Vec<&GemSource> = sources
            .iter()
            .filter(|(j, s)| *j == i && s.kind == GemSourceKind::Vendor)
            .map(|(_, s)| *s)
            .collect();
        if let Some(s) = vendors
            .iter()
            .find(|s| s.act >= need_act)
            .or_else(|| vendors.last())
        {
            placed.insert(i);
            steps.push(step(w, s, GemAction::Buy));
        }
    }

    // Reward-only gems whose every offer went to another gem.
    for (i, w) in wanted.iter().enumerate() {
        if !placed.contains(&i) && !not_in_campaign.contains(&w.name) {
            not_in_campaign.push(w.name.clone());
        }
    }

    steps.sort_by_key(|st| {
        let order = sources
            .iter()
            .find(|(_, s)| s.quest_id == st.quest_id)
            .map(|(_, s)| quest_order(s))
            .unwrap_or((u32::MAX, u32::MAX));
        (order, st.action == GemAction::Buy)
    });
    GemPlan {
        steps,
        not_in_campaign,
    }
}

/// Puts the plan's gems right after the step that hands in their quest. Vendors sit in the same
/// town, so the buys go there too.
pub fn annotate(view: &mut RouteView, plan: &GemPlan, hints: GemHints) {
    let mut out = Vec::with_capacity(view.here.len());
    for step in view.here.drain(..) {
        let quests: Vec<String> = step
            .parts
            .iter()
            .filter_map(|p| match p {
                Part::Quest { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        out.push(step);
        for quest in &quests {
            out.extend(
                plan.for_quest(quest)
                    .filter(|g| hints.shows(g.action))
                    .map(gem_step),
            );
        }
    }
    view.here = out;
}

fn gem_step(g: &GemStep) -> Step {
    let parts = match g.action {
        GemAction::Take => vec![
            Part::Text {
                text: "Take ".into(),
            },
            Part::RewardQuest {
                item: g.name.clone(),
            },
        ],
        GemAction::Buy => vec![
            Part::Text {
                text: "Buy ".into(),
            },
            Part::RewardVendor {
                item: g.name.clone(),
                cost: None,
            },
            Part::Text {
                text: format!(" from {}", g.npc),
            },
        ],
    };
    Step {
        parts,
        sub_steps: Vec::new(),
        edge: None,
    }
}

fn step(w: &Wanted, s: &GemSource, action: GemAction) -> GemStep {
    GemStep {
        gem_id: w.id.clone(),
        name: w.name.clone(),
        quest_id: s.quest_id.clone(),
        act: s.act,
        action,
        npc: s.npc.clone(),
    }
}

#[cfg(test)]
mod tests {
    use exile_pob::{Gem, SkillGroup, SkillSet};

    use super::*;

    const GEMS: &str = r#"{
      "A": {"id": "A", "name": "Alpha", "required_level": 1, "is_support": false},
      "B": {"id": "B", "name": "Beta", "required_level": 1, "is_support": false},
      "S": {"id": "S", "name": "Support", "required_level": 1, "is_support": true},
      "D": {"id": "D", "name": "Drop Only", "required_level": 1, "is_support": false}
    }"#;

    const QUESTS: &str = r#"{
      "a1q1": {"id": "a1q1", "name": "First", "act": "1", "reward_offers": {"a1q1": {
        "quest_npc": "Tarkleigh",
        "quest": {"A": {"classes": ["Witch"]}, "B": {"classes": ["Witch"]}, "S": {"classes": ["Witch"]}},
        "vendor": {"S": {"classes": ["Witch"], "npc": "Nessa"}}
      }}},
      "a1q2": {"id": "a1q2", "name": "Second", "act": "1", "reward_offers": {"a1q2": {
        "quest_npc": "Tarkleigh",
        "quest": {"B": {"classes": ["Witch"]}},
        "vendor": {"A": {"classes": ["Witch"], "npc": "Nessa"}, "B": {"classes": ["Witch"], "npc": "Nessa"}}
      }}},
      "a10q1": {"id": "a10q1", "name": "Late", "act": "10", "reward_offers": {"a10q1": {
        "quest_npc": "Lani",
        "quest": {"A": {"classes": ["Witch"]}}
      }}}
    }"#;

    fn table() -> GemTable {
        GemTable::from_json_bytes(GEMS.as_bytes(), QUESTS.as_bytes()).unwrap()
    }

    fn gem(id: &str, support: bool) -> Gem {
        Gem {
            name_spec: Some(id.to_string()),
            gem_id: Some(id.to_string()),
            enabled: true,
            is_support: support,
            ..Gem::default()
        }
    }

    fn build(groups: Vec<SkillGroup>) -> PobBuild {
        PobBuild {
            skill_sets: vec![SkillSet {
                groups,
                ..SkillSet::default()
            }],
            ..PobBuild::default()
        }
    }

    fn group(gems: Vec<Gem>) -> SkillGroup {
        SkillGroup {
            enabled: true,
            gems,
            ..SkillGroup::default()
        }
    }

    fn summary(p: &GemPlan) -> Vec<(String, String, GemAction)> {
        p.steps
            .iter()
            .map(|s| (s.quest_id.clone(), s.gem_id.clone(), s.action))
            .collect()
    }

    #[test]
    fn one_reward_per_quest_then_vendors() {
        let p = plan(
            &build(vec![group(vec![gem("A", false), gem("B", false)])]),
            "Witch",
            &table(),
        );
        assert_eq!(
            summary(&p),
            [
                ("a1q1".into(), "A".into(), GemAction::Take),
                ("a1q2".into(), "B".into(), GemAction::Take),
            ]
        );
    }

    #[test]
    fn active_gem_wins_the_reward_over_a_support_the_vendor_sells() {
        let p = plan(
            &build(vec![group(vec![gem("S", true), gem("A", false)])]),
            "Witch",
            &table(),
        );
        assert_eq!(
            summary(&p),
            [
                ("a1q1".into(), "A".into(), GemAction::Take),
                ("a1q1".into(), "S".into(), GemAction::Buy),
            ]
        );
    }

    #[test]
    fn a_gem_without_a_reward_is_bought_at_the_first_vendor() {
        let p = plan(
            &build(vec![group(vec![
                gem("A", false),
                gem("B", false),
                gem("S", true),
            ])]),
            "Witch",
            &table(),
        );
        let s = summary(&p);
        assert!(s.contains(&("a1q1".into(), "A".into(), GemAction::Take)));
        assert!(s.contains(&("a1q1".into(), "S".into(), GemAction::Buy)));
        assert!(s.contains(&("a1q2".into(), "B".into(), GemAction::Take)));
    }

    #[test]
    fn each_offer_of_a_quest_is_its_own_reward() {
        let gems = r#"{
          "A": {"id": "A", "name": "A", "required_level": 1, "is_support": false},
          "B": {"id": "B", "name": "B", "required_level": 1, "is_support": false},
          "C": {"id": "C", "name": "C", "required_level": 1, "is_support": false}
        }"#;
        let quests = r#"{"a1q4": {"id": "a1q4", "name": "Eggs", "act": "1", "reward_offers": {
          "a1q4": {"quest_npc": "Tarkleigh", "quest": {"A": {"classes": ["Ranger"]}, "B": {"classes": ["Ranger"]}}},
          "a1q4b": {"quest_npc": "Tarkleigh", "quest": {"C": {"classes": ["Ranger"]}}}
        }}}"#;
        let t = GemTable::from_json_bytes(gems.as_bytes(), quests.as_bytes()).unwrap();
        let p = plan(
            &build(vec![group(vec![
                gem("A", false),
                gem("B", false),
                gem("C", false),
            ])]),
            "Ranger",
            &t,
        );
        assert_eq!(
            summary(&p),
            [
                ("a1q4".into(), "A".into(), GemAction::Take),
                ("a1q4".into(), "C".into(), GemAction::Take),
            ]
        );
        // B lost its only reward to A and no vendor sells it.
        assert_eq!(p.not_in_campaign, ["B"]);
    }

    #[test]
    fn gems_the_campaign_never_gives_are_listed_apart() {
        let p = plan(
            &build(vec![group(vec![gem("A", false), gem("D", false)])]),
            "Witch",
            &table(),
        );
        assert_eq!(p.not_in_campaign, ["D"]);
    }

    #[test]
    fn disabled_gems_groups_and_item_skills_are_skipped() {
        let mut off = gem("B", false);
        off.enabled = false;
        let mut disabled_group = group(vec![gem("S", true)]);
        disabled_group.enabled = false;
        let mut item_skill = group(vec![gem("D", false)]);
        item_skill.source = Some("Item:1:Some Unique".into());
        let p = plan(
            &build(vec![
                group(vec![gem("A", false), off]),
                disabled_group,
                item_skill,
            ]),
            "Witch",
            &table(),
        );
        assert_eq!(summary(&p), [("a1q1".into(), "A".into(), GemAction::Take)]);
        assert!(p.not_in_campaign.is_empty());
    }

    #[test]
    fn the_same_gem_in_two_groups_is_planned_once() {
        let p = plan(
            &build(vec![
                group(vec![gem("A", false)]),
                group(vec![gem("A", false)]),
            ]),
            "Witch",
            &table(),
        );
        assert_eq!(p.steps.len(), 1);
    }

    #[test]
    fn another_class_gets_nothing_from_witch_rewards() {
        let p = plan(
            &build(vec![group(vec![gem("A", false)])]),
            "Marauder",
            &table(),
        );
        assert!(p.steps.is_empty());
        assert_eq!(p.not_in_campaign, ["A"]);
    }

    fn view_with(here: Vec<Step>) -> RouteView {
        RouteView {
            section: "Act 1".into(),
            edge: 1,
            edge_count: 2,
            section_edge: 1,
            section_edge_count: 2,
            finished_section: None,
            area_id: "1_1_town".into(),
            here,
            then: None,
        }
    }

    fn text_of(step: &Step) -> String {
        step.parts
            .iter()
            .map(|p| match p {
                Part::Text { text } => text.clone(),
                Part::RewardQuest { item } | Part::RewardVendor { item, .. } => item.clone(),
                Part::Quest { name, .. } => name.clone(),
                _ => String::new(),
            })
            .collect()
    }

    fn hand_in_lines(hints: GemHints) -> Vec<String> {
        let p = plan(
            &build(vec![group(vec![gem("S", true), gem("A", false)])]),
            "Witch",
            &table(),
        );
        let hand_in = Step {
            parts: vec![
                Part::Text {
                    text: "Hand in ".into(),
                },
                Part::Quest {
                    id: "a1q1".into(),
                    name: "First".into(),
                },
            ],
            sub_steps: Vec::new(),
            edge: None,
        };
        let other = Step {
            parts: vec![Part::Waypoint],
            sub_steps: Vec::new(),
            edge: None,
        };
        let mut view = view_with(vec![hand_in, other]);
        annotate(&mut view, &p, hints);
        view.here.iter().map(text_of).collect()
    }

    #[test]
    fn gems_follow_the_quest_hand_in() {
        assert_eq!(
            hand_in_lines(GemHints::RewardsAndVendors),
            ["Hand in First", "Take A", "Buy S from Nessa", ""]
        );
    }

    #[test]
    fn rewards_only_drops_vendor_buys() {
        assert_eq!(
            hand_in_lines(GemHints::Rewards),
            ["Hand in First", "Take A", ""]
        );
    }

    #[test]
    fn off_leaves_the_route_as_is() {
        assert_eq!(hand_in_lines(GemHints::Off), ["Hand in First", ""]);
    }

    #[test]
    fn hints_read_the_settings_names() {
        let h: GemHints = serde_json::from_str(r#""rewards""#).unwrap();
        assert_eq!(h, GemHints::Rewards);
        assert_eq!(
            serde_json::to_string(&GemHints::RewardsAndVendors).unwrap(),
            r#""rewardsAndVendors""#
        );
    }

    #[test]
    fn a_view_without_the_quest_is_left_alone() {
        let p = plan(
            &build(vec![group(vec![gem("A", false)])]),
            "Witch",
            &table(),
        );
        let step = Step {
            parts: vec![Part::Waypoint],
            sub_steps: Vec::new(),
            edge: None,
        };
        let mut view = view_with(vec![step.clone()]);
        annotate(&mut view, &p, GemHints::RewardsAndVendors);
        assert_eq!(view.here, [step]);
    }

    // The first town offers both the starting skill and the one the build switches to later.
    const STAGED_GEMS: &str = r#"{
      "Start": {"id": "Start", "name": "Start", "required_level": 1, "is_support": false},
      "Late": {"id": "Late", "name": "Late", "required_level": 28, "is_support": false},
      "Boss": {"id": "Boss", "name": "Boss", "required_level": 1, "is_support": false}
    }"#;

    const STAGED_QUESTS: &str = r#"{
      "a1q1": {"id": "a1q1", "name": "First", "act": "1", "reward_offers": {"a1q1": {
        "quest_npc": "Tarkleigh",
        "quest": {"Start": {"classes": ["Witch"]}, "Late": {"classes": ["Witch"]}, "Boss": {"classes": ["Witch"]}},
        "vendor": {"Late": {"classes": ["Witch"], "npc": "Nessa"}, "Boss": {"classes": ["Witch"], "npc": "Nessa"}}
      }}},
      "a4q1": {"id": "a4q1", "name": "Fourth", "act": "4", "reward_offers": {"a4q1": {
        "quest_npc": "Oyun",
        "vendor": {"Late": {"classes": ["Witch"], "npc": "Petarus and Vanja"}, "Boss": {"classes": ["Witch"], "npc": "Petarus and Vanja"}}
      }}}
    }"#;

    fn staged() -> GemTable {
        GemTable::from_json_bytes(STAGED_GEMS.as_bytes(), STAGED_QUESTS.as_bytes()).unwrap()
    }

    fn set(title: &str, gems: Vec<Gem>) -> SkillSet {
        SkillSet {
            title: (!title.is_empty()).then(|| title.to_string()),
            groups: if gems.is_empty() {
                Vec::new()
            } else {
                vec![group(gems)]
            },
            ..SkillSet::default()
        }
    }

    fn sets(sets: Vec<SkillSet>) -> PobBuild {
        PobBuild {
            skill_sets: sets,
            ..PobBuild::default()
        }
    }

    #[test]
    fn act_presets_keep_the_later_skill_out_of_the_first_reward() {
        let b = sets(vec![
            set("Act 1", vec![gem("Start", false)]),
            set("Act 4 switch", vec![gem("Late", false)]),
        ]);
        let p = plan(&b, "Witch", &staged());
        assert_eq!(
            summary(&p),
            [
                ("a1q1".into(), "Start".into(), GemAction::Take),
                ("a4q1".into(), "Late".into(), GemAction::Buy),
            ]
        );
    }

    #[test]
    fn untitled_presets_count_in_order_and_by_gem_level() {
        let b = sets(vec![
            set("whatever", vec![gem("Start", false)]),
            set("--- separator ---", vec![]),
            set("my cool setup", vec![gem("Late", false)]),
        ]);
        let p = plan(&b, "Witch", &staged());
        assert_eq!(
            summary(&p),
            [
                ("a1q1".into(), "Start".into(), GemAction::Take),
                ("a4q1".into(), "Late".into(), GemAction::Buy),
            ]
        );
    }

    fn untitled_two_sets() -> PobBuild {
        sets(vec![
            set("my setup", vec![gem("Start", false)]),
            set("other one", vec![gem("Boss", false)]),
        ])
    }

    #[test]
    fn a_stage_set_by_hand_moves_the_gem_to_that_act() {
        let auto = plan(&untitled_two_sets(), "Witch", &staged());
        assert!(summary(&auto).contains(&("a1q1".into(), "Boss".into(), GemAction::Buy)));

        let stages = [PresetStage::Auto, PresetStage::Act(4)];
        let p = plan_with_stages(&untitled_two_sets(), "Witch", &staged(), &stages);
        assert_eq!(
            summary(&p),
            [
                ("a1q1".into(), "Start".into(), GemAction::Take),
                ("a4q1".into(), "Boss".into(), GemAction::Buy),
            ]
        );
    }

    #[test]
    fn a_skipped_set_leaves_the_plan() {
        let stages = [PresetStage::Auto, PresetStage::Skip];
        let p = plan_with_stages(&untitled_two_sets(), "Witch", &staged(), &stages);
        assert_eq!(
            summary(&p),
            [("a1q1".into(), "Start".into(), GemAction::Take)]
        );
        assert!(p.not_in_campaign.is_empty());
    }

    #[test]
    fn stages_carry_over_only_to_the_same_skill_sets() {
        let with = |titles: &[&str]| PobBuild {
            skill_sets: titles
                .iter()
                .map(|t| SkillSet {
                    id: None,
                    title: Some(t.to_string()),
                    groups: Vec::new(),
                })
                .collect(),
            ..PobBuild::default()
        };
        assert!(same_presets(
            &with(&["Act 1", "Act 4"]),
            &with(&["Act 1", "Act 4"])
        ));
        assert!(!same_presets(&with(&["Act 1", "Act 4"]), &with(&["Act 1"])));
        assert!(!same_presets(&with(&["Act 1"]), &with(&["Leveling"])));
    }

    #[test]
    fn presets_report_what_the_planner_reads_from_them() {
        let b = sets(vec![
            set("whatever", vec![gem("Start", false)]),
            set("--- separator ---", vec![]),
            set("Act 3", vec![gem("Late", false), gem("Boss", false)]),
            set("later", vec![gem("Boss", false)]),
        ]);
        let got: Vec<_> = presets(&b)
            .into_iter()
            .map(|p| (p.title.unwrap_or_default(), p.gem_count, p.auto_level))
            .collect();
        let act3 = act_expected_level(3);
        assert_eq!(
            got,
            [
                ("whatever".to_string(), 1, None),
                ("--- separator ---".to_string(), 0, None),
                ("Act 3".to_string(), 2, Some(act3)),
                ("later".to_string(), 1, Some(act3)),
            ]
        );
    }

    #[test]
    fn stages_read_the_settings_names() {
        let s: Vec<PresetStage> =
            serde_json::from_str(r#"["auto", {"act": 3}, "endgame", "skip"]"#).unwrap();
        assert_eq!(
            s,
            [
                PresetStage::Auto,
                PresetStage::Act(3),
                PresetStage::Endgame,
                PresetStage::Skip
            ]
        );
    }

    #[test]
    fn a_single_final_preset_rewards_the_gem_usable_first() {
        let b = sets(vec![set("", vec![gem("Late", false), gem("Start", false)])]);
        let p = plan(&b, "Witch", &staged());
        assert_eq!(p.steps[0].gem_id, "Start");
        assert_eq!(p.steps[0].action, GemAction::Take);
    }

    #[test]
    fn endgame_presets_buy_late() {
        let b = sets(vec![
            set("Act 1", vec![gem("Start", false)]),
            set("Bossing", vec![gem("Boss", false)]),
        ]);
        let p = plan(&b, "Witch", &staged());
        assert!(summary(&p).contains(&("a4q1".into(), "Boss".into(), GemAction::Buy)));
    }

    #[test]
    fn plans_a_witch_build_against_the_real_data() {
        let gems = real_table();
        let p = plan(
            &build(vec![group(vec![
                gem("Metadata/Items/Gems/SkillGemRaiseZombie", false),
                gem("Metadata/Items/Gems/SupportGemArcaneSurge", true),
            ])]),
            "Witch",
            &gems,
        );
        let first = &p.steps[0];
        assert_eq!(first.quest_id, "a1q1");
        assert_eq!(first.gem_id, "Metadata/Items/Gems/SkillGemRaiseZombie");
        assert_eq!(first.action, GemAction::Take);
        assert!(p
            .steps
            .iter()
            .any(|s| s.gem_id.ends_with("ArcaneSurge") && s.action == GemAction::Buy));
    }

    fn real_table() -> GemTable {
        GemTable::from_json_bytes(
            include_bytes!("../../data/data/routes/exile-leveling/gems.json"),
            include_bytes!("../../data/data/routes/exile-leveling/quests.json"),
        )
        .unwrap()
    }

    #[test]
    fn each_class_gets_raise_zombie_its_own_way() {
        let gems = real_table();
        let zombie = || {
            build(vec![group(vec![gem(
                "Metadata/Items/Gems/SkillGemRaiseZombie",
                false,
            )])])
        };
        let got: Vec<(&str, String, GemAction, String)> = [
            "Witch", "Templar", "Scion", "Marauder", "Duelist", "Ranger", "Shadow",
        ]
        .into_iter()
        .map(|class| {
            let p = plan(&zombie(), class, &gems);
            assert!(p.not_in_campaign.is_empty(), "{class}");
            assert_eq!(p.steps.len(), 1, "{class}");
            let s = &p.steps[0];
            (class, s.quest_id.clone(), s.action, s.npc.clone())
        })
        .collect();
        let take = GemAction::Take;
        let buy = GemAction::Buy;
        assert_eq!(
            got,
            [
                ("Witch", "a1q1".into(), take, "Tarkleigh".into()),
                ("Templar", "a1q1".into(), buy, "Nessa".into()),
                ("Scion", "a1q1".into(), buy, "Nessa".into()),
                ("Marauder", "a3q12".into(), buy, "Siosa".into()),
                ("Duelist", "a3q12".into(), buy, "Siosa".into()),
                ("Ranger", "a3q12".into(), buy, "Siosa".into()),
                ("Shadow", "a3q12".into(), buy, "Siosa".into()),
            ]
        );
    }
}
