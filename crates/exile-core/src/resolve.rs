//! Resolvers that map the current campaign state onto the imported build.
//!
//! - [`TreeResolver`] picks which of a build's passive-tree specs best matches the player's current
//!   progress, using only the specs' free-text titles (the KEY FACT: a spec carries no level/act),
//!   and falls back to the build's `activeSpec` when no title gives a usable signal.
//! - [`GemResolver`] finds the gems a build uses that are claimable as quest rewards around the
//!   player's current act, filtered to the player's class.

use std::collections::HashSet;

use exile_pob::{PobBuild, TreeSpec};

use crate::data::DataPack;
use crate::state::CampaignState;

/// Roughly the character level a fresh character reaches at the start of each act. Used to compare a
/// spec titled by act against a player tracked by level, and vice versa. Monotonic; approximate.
pub fn act_expected_level(act: u8) -> u32 {
    match act {
        0 | 1 => 1,
        2 => 12,
        3 => 22,
        4 => 32,
        5 => 40,
        6 => 45,
        7 => 50,
        8 => 54,
        9 => 60,
        _ => 64, // act 10+
    }
}

/// A spec's leveling target, distilled from its title into a single comparable level.
fn spec_target_level(spec: &TreeSpec) -> Option<u32> {
    title_target_level(spec.title.as_deref()?)
}

/// The level a free-text PoB title ("Act 3", "lvl 28", "Endgame") points at; 100 for endgame.
pub(crate) fn title_target_level(title: &str) -> Option<u32> {
    let title = title.to_ascii_lowercase();

    // Endgame-ish specs sit above the whole campaign.
    if [
        "endgame",
        "end game",
        "maps",
        "mapping",
        "pinnacle",
        "uber",
        "aspirational",
        "boss",
        "delve",
        "sanctum",
        "heist",
    ]
    .iter()
    .any(|kw| title.contains(kw))
    {
        return Some(100);
    }

    // "level 68" / "lvl 68" / "lv 68".
    if let Some(level) = find_after_keyword(&title, &["level", "lvl", "lv"]) {
        return Some(level);
    }

    // "act 3" / "(act 3)".
    if let Some(act) = find_after_keyword(&title, &["act"]) {
        return Some(act_expected_level(act.min(10) as u8));
    }

    // A leaguestart / start-of-campaign label.
    if title.contains("leaguestart") || title.contains("league start") || title.contains("start") {
        return Some(1);
    }

    None
}

/// Find the first integer that follows any of `keywords` in `text`, e.g. keyword "level" in
/// "level 68 (endgame)" -> 68. Skips spaces and a possible ':' between the keyword and the number.
fn find_after_keyword(text: &str, keywords: &[&str]) -> Option<u32> {
    for kw in keywords {
        let mut from = 0;
        while let Some(pos) = text[from..].find(kw) {
            let after = from + pos + kw.len();
            let rest = text[after..].trim_start_matches([' ', ':', '.', '(']);
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<u32>() {
                return Some(n);
            }
            from = after;
        }
    }
    None
}

/// Why a particular spec was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecReason {
    /// The build has a single spec.
    OnlySpec,
    /// Chosen because its title matched the player's progress.
    TitleMatch,
    /// No title gave a usable signal; fell back to the build's `activeSpec`.
    ActiveSpecFallback,
}

/// The resolved current spec.
#[derive(Debug, Clone)]
pub struct ResolvedSpec<'a> {
    /// 0-based index into `build.specs`.
    pub index: usize,
    pub spec: &'a TreeSpec,
    pub reason: SpecReason,
}

pub struct TreeResolver;

impl TreeResolver {
    /// Pick the spec that best matches the current progress. `None` only if the build has no specs.
    pub fn resolve<'a>(state: &CampaignState, build: &'a PobBuild) -> Option<ResolvedSpec<'a>> {
        if build.specs.is_empty() {
            return None;
        }
        if build.specs.len() == 1 {
            return Some(ResolvedSpec {
                index: 0,
                spec: &build.specs[0],
                reason: SpecReason::OnlySpec,
            });
        }

        // The player's progress as a single level (max of the tracked level and the act's baseline).
        let progress = state
            .level
            .max(state.act.map(act_expected_level).unwrap_or(0));

        // Specs whose title yields a target level.
        let targeted: Vec<(usize, u32)> = build
            .specs
            .iter()
            .enumerate()
            .filter_map(|(i, s)| spec_target_level(s).map(|t| (i, t)))
            .collect();

        if let Some(index) = pick_by_progress(&targeted, progress) {
            return Some(ResolvedSpec {
                index,
                spec: &build.specs[index],
                reason: SpecReason::TitleMatch,
            });
        }

        // No title signal: fall back to the selected spec (activeSpec, 1-based).
        let index = build
            .active_spec
            .saturating_sub(1)
            .min(build.specs.len() - 1);
        Some(ResolvedSpec {
            index,
            spec: &build.specs[index],
            reason: SpecReason::ActiveSpecFallback,
        })
    }
}

/// From (index, target-level) pairs, choose the most-advanced spec the player has reached (greatest
/// target <= progress); if the player is below every target, choose the earliest (lowest target).
fn pick_by_progress(targeted: &[(usize, u32)], progress: u32) -> Option<usize> {
    if targeted.is_empty() {
        return None;
    }
    let reached = targeted
        .iter()
        .filter(|(_, t)| *t <= progress)
        .max_by_key(|(_, t)| *t);
    match reached {
        Some((i, _)) => Some(*i),
        None => targeted.iter().min_by_key(|(_, t)| *t).map(|(i, _)| *i),
    }
}

/// A gem the build uses that a quest can reward at the current stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GemPick {
    pub gem_id: String,
    pub gem_name: String,
    pub quest_name: String,
    /// The NPC you claim it from (e.g. "Tarkleigh"); `None` if unknown.
    pub npc: Option<String>,
    pub act: i32,
    pub reward_level: u32,
}

pub struct GemResolver;

impl GemResolver {
    /// Gems the build uses that are rewarded by a quest in the player's current act and claimable by
    /// the player's class. Empty when there is no current act or no matching gem.
    pub fn available_now(state: &CampaignState, data: &DataPack, build: &PobBuild) -> Vec<GemPick> {
        let Some(act) = state.act.map(i32::from) else {
            return Vec::new();
        };
        // The class the reward must allow: prefer the build's class, else the logged class.
        let class = build
            .class_name
            .as_deref()
            .or(state.class.as_deref())
            .unwrap_or("");

        // Gem ids the build actually uses.
        let build_gem_ids: HashSet<&str> =
            build.gems().filter_map(|g| g.gem_id.as_deref()).collect();

        let mut picks = Vec::new();
        let mut seen = HashSet::new();
        for gem_id in &build_gem_ids {
            for (quest, reward) in data.quests.rewarding_gem(gem_id) {
                if quest.act == act && reward.allows_class(class) && seen.insert(*gem_id) {
                    picks.push(GemPick {
                        gem_id: reward.gem_id.clone(),
                        gem_name: reward.gem_name.clone(),
                        quest_name: quest.name.clone(),
                        npc: quest.npc.clone(),
                        act: quest.act,
                        reward_level: reward.reward_level,
                    });
                }
            }
        }
        // Deterministic order for a stable UI.
        picks.sort_by(|a, b| a.gem_name.cmp(&b.gem_name));
        picks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exile_pob::{Gem, SkillGroup, SkillSet};

    fn spec(title: Option<&str>) -> TreeSpec {
        TreeSpec {
            title: title.map(str::to_string),
            ..TreeSpec::default()
        }
    }

    fn build_with_specs(active: usize, titles: &[Option<&str>]) -> PobBuild {
        PobBuild {
            active_spec: active,
            specs: titles.iter().map(|t| spec(*t)).collect(),
            ..PobBuild::default()
        }
    }

    fn state_at(level: u32, act: Option<u8>) -> CampaignState {
        CampaignState {
            level,
            act,
            ..CampaignState::default()
        }
    }

    #[test]
    fn parses_level_and_act_titles() {
        assert_eq!(
            spec_target_level(&spec(Some("Level 68 (Endgame)"))),
            Some(100)
        );
        assert_eq!(spec_target_level(&spec(Some("Level 31"))), Some(31));
        assert_eq!(spec_target_level(&spec(Some("lvl 12"))), Some(12));
        assert_eq!(spec_target_level(&spec(Some("Act 3"))), Some(22));
        assert_eq!(spec_target_level(&spec(Some("Leaguestart"))), Some(1));
        assert_eq!(spec_target_level(&spec(Some("My cool tree"))), None);
        assert_eq!(spec_target_level(&spec(None)), None);
    }

    #[test]
    fn single_spec_is_only_spec() {
        let build = build_with_specs(1, &[Some("Whatever")]);
        let r = TreeResolver::resolve(&state_at(1, Some(1)), &build).unwrap();
        assert_eq!(r.index, 0);
        assert_eq!(r.reason, SpecReason::OnlySpec);
    }

    #[test]
    fn picks_most_advanced_reached_spec() {
        let build = build_with_specs(1, &[Some("Level 12"), Some("Level 30"), Some("Endgame")]);
        // Level 35 -> reached Level 30 (not Endgame).
        let r = TreeResolver::resolve(&state_at(35, Some(4)), &build).unwrap();
        assert_eq!(r.index, 1);
        assert_eq!(r.reason, SpecReason::TitleMatch);
    }

    #[test]
    fn below_all_targets_picks_earliest() {
        let build = build_with_specs(2, &[Some("Level 30"), Some("Level 68"), Some("Endgame")]);
        // Fresh character, below every target -> the earliest spec (Level 30).
        let r = TreeResolver::resolve(&state_at(1, Some(1)), &build).unwrap();
        assert_eq!(r.index, 0);
        assert_eq!(r.reason, SpecReason::TitleMatch);
    }

    #[test]
    fn untitled_specs_fall_back_to_active_spec() {
        let build = build_with_specs(2, &[None, Some("Default"), None]);
        // "Default" contains no level/act/keyword -> no title signal -> activeSpec (1-based 2 -> idx 1).
        let r = TreeResolver::resolve(&state_at(40, Some(5)), &build).unwrap();
        assert_eq!(r.index, 1);
        assert_eq!(r.reason, SpecReason::ActiveSpecFallback);
    }

    #[test]
    fn no_specs_resolves_to_none() {
        let build = PobBuild::default();
        assert!(TreeResolver::resolve(&state_at(1, Some(1)), &build).is_none());
    }

    // --- Gem resolver ---

    const PACK: &str = r#"{
        "schema_version":1,"patch":"t","generated_at":"t","source":"t","areas":[]
    }"#;
    const QUESTS: &str = r#"{
        "schema_version":1,"patch":"t","generated_at":"t","source":"t","quests":[
            {"id":"a1q1","act":1,"name":"Enemy at the Gate","npc":"Tarkleigh","gem_rewards":[
                {"gem_id":"g/Freezing","gem_name":"Freezing Pulse","reward_level":1,"classes":["Witch"]},
                {"gem_id":"g/Cleave","gem_name":"Cleave","reward_level":1,"classes":["Duelist"]}
            ]},
            {"id":"a2q1","act":2,"name":"Intruders","gem_rewards":[
                {"gem_id":"g/Herald","gem_name":"Herald of Ash","reward_level":1,"classes":[]}
            ]}
        ]
    }"#;
    const TRIALS: &str = r#"{
        "schema_version":1,"patch":"t","generated_at":"t","source":"t","trials":[]
    }"#;

    fn pack() -> DataPack {
        DataPack::from_bytes(
            PACK.as_bytes(),
            1,
            QUESTS.as_bytes(),
            1,
            TRIALS.as_bytes(),
            1,
        )
        .expect("pack")
    }

    fn build_using(gem_ids: &[&str], class: &str) -> PobBuild {
        let gems = gem_ids
            .iter()
            .map(|id| Gem {
                gem_id: Some(id.to_string()),
                enabled: true,
                ..Gem::default()
            })
            .collect();
        PobBuild {
            class_name: Some(class.to_string()),
            skill_sets: vec![SkillSet {
                groups: vec![SkillGroup {
                    enabled: true,
                    gems,
                    ..SkillGroup::default()
                }],
                ..SkillSet::default()
            }],
            ..PobBuild::default()
        }
    }

    #[test]
    fn gem_resolver_surfaces_current_act_build_gems_for_class() {
        let data = pack();
        let build = build_using(&["g/Freezing", "g/Herald", "g/Unused"], "Witch");
        // In act 1: Freezing Pulse is a Witch reward here; Cleave is Duelist-only; Herald is act 2.
        let picks = GemResolver::available_now(&state_at(3, Some(1)), &data, &build);
        assert_eq!(picks.len(), 1);
        assert_eq!(picks[0].gem_name, "Freezing Pulse");
        assert_eq!(picks[0].quest_name, "Enemy at the Gate");
        assert_eq!(picks[0].npc.as_deref(), Some("Tarkleigh"));
    }

    #[test]
    fn gem_resolver_respects_class_filter() {
        let data = pack();
        // A Duelist build using Cleave gets it in act 1; a Witch using Cleave would not.
        let duelist = build_using(&["g/Cleave"], "Duelist");
        assert_eq!(
            GemResolver::available_now(&state_at(1, Some(1)), &data, &duelist).len(),
            1
        );
        let witch = build_using(&["g/Cleave"], "Witch");
        assert!(GemResolver::available_now(&state_at(1, Some(1)), &data, &witch).is_empty());
    }

    #[test]
    fn gem_resolver_empty_without_act() {
        let data = pack();
        let build = build_using(&["g/Freezing"], "Witch");
        assert!(GemResolver::available_now(&state_at(3, None), &data, &build).is_empty());
    }
}
