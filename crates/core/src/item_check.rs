//! An item the player copied in the game with Ctrl+C, and what a gem means for their build.

use exile_pob::PobBuild;
use serde::Serialize;

use crate::gemplan::{GemAction, GemPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiedItem {
    pub class: String,
    pub rarity: String,
    pub name: String,
    /// The base type under a rare's or unique's name.
    pub base: Option<String>,
    /// From the item's Requirements block. A gem's own "Level:" line is its gem level, not this.
    pub required_level: Option<u32>,
}

impl CopiedItem {
    pub fn is_gem(&self) -> bool {
        self.rarity == "Gem"
    }

    /// "Death Song, Short Bow" for a rare, the name alone otherwise.
    pub fn title(&self) -> String {
        match &self.base {
            Some(base) => format!("{}, {base}", self.name),
            None => self.name.clone(),
        }
    }
}

/// The text the game puts on the clipboard for an item, English client. Anything else is None.
pub fn parse(text: &str) -> Option<CopiedItem> {
    let mut lines = text.lines().map(str::trim);
    let class = lines.next()?.strip_prefix("Item Class: ")?.to_string();
    let rarity = lines.next()?.strip_prefix("Rarity: ")?.to_string();
    let name = lines
        .next()
        .filter(|l| !l.is_empty() && !l.starts_with("---"))?;
    let base = lines
        .next()
        .filter(|l| !l.is_empty() && !l.starts_with("---"))
        .map(str::to_string);
    Some(CopiedItem {
        class,
        rarity,
        name: name.to_string(),
        base,
        required_level: required_level(text),
    })
}

fn required_level(text: &str) -> Option<u32> {
    text.lines()
        .map(str::trim)
        .skip_while(|l| *l != "Requirements:")
        .skip(1)
        .take_while(|l| !l.starts_with("---"))
        .find_map(|l| l.strip_prefix("Level: "))
        .and_then(|l| {
            l.split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|n| n.parse().ok())
        })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GemUse {
    /// The skill set's title, when the build has more than one.
    pub set: Option<String>,
    /// The group's label, else its first active gem.
    pub skill: String,
    /// The other enabled gems in the group.
    pub links: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GemCard {
    pub name: String,
    pub uses: Vec<GemUse>,
    /// Where the campaign gives it to this class, from the gem plan.
    pub get: Option<String>,
}

const MAX_USES: usize = 4;

// PoB names supports without " Support", the game with it.
fn same_gem(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        let s = s.trim().to_lowercase();
        s.strip_suffix(" support").map(str::to_string).unwrap_or(s)
    };
    norm(a) == norm(b)
}

/// Which socket groups of the build use the gem, and where the campaign gives it.
pub fn gem_card(build: &PobBuild, plan: Option<&GemPlan>, name: &str) -> GemCard {
    let many_sets = build.skill_sets.len() > 1;
    let mut uses: Vec<GemUse> = Vec::new();
    for set in &build.skill_sets {
        for group in set.groups.iter().filter(|g| g.enabled) {
            let gem_name = |g: &exile_pob::Gem| g.name_spec.clone().unwrap_or_default();
            if !group
                .gems
                .iter()
                .any(|g| g.enabled && same_gem(&gem_name(g), name))
            {
                continue;
            }
            let skill = group
                .label
                .clone()
                .filter(|l| !l.trim().is_empty())
                .or_else(|| group.first_active_gem().map(gem_name))
                .unwrap_or_default();
            let links: Vec<String> = group
                .gems
                .iter()
                .filter(|g| g.enabled)
                .map(gem_name)
                .filter(|n| !n.is_empty() && !same_gem(n, name) && *n != skill)
                .collect();
            if uses.iter().any(|u| u.skill == skill && u.links == links) {
                continue;
            }
            uses.push(GemUse {
                set: set.title.clone().filter(|_| many_sets),
                skill,
                links,
            });
        }
    }
    uses.truncate(MAX_USES);
    let get = plan
        .and_then(|p| p.steps.iter().find(|s| same_gem(&s.name, name)))
        .map(|s| match s.action {
            GemAction::Take => format!("Quest reward from {} in act {}", s.npc, s.act),
            GemAction::Buy => format!("Sold by {} in act {}", s.npc, s.act),
        });
    GemCard {
        name: name.to_string(),
        uses,
        get,
    }
}

#[cfg(test)]
mod tests {
    use exile_pob::{Gem, SkillGroup, SkillSet};

    use super::*;
    use crate::gemplan::GemStep;

    const BOW: &str = "Item Class: Bows\nRarity: Rare\nDeath Song\nShort Bow\n--------\nItem Level: 30\n--------\nAdds 10 to 20 Physical Damage";
    const GEM: &str = "Item Class: Support Gems\nRarity: Gem\nAdded Fire Damage Support\n--------\nSupport, Fire\nLevel: 1";

    fn gem(name: &str, support: bool) -> Gem {
        Gem {
            name_spec: Some(name.into()),
            enabled: true,
            is_support: support,
            ..Default::default()
        }
    }

    fn group(label: Option<&str>, gems: Vec<Gem>) -> SkillGroup {
        SkillGroup {
            enabled: true,
            label: label.map(str::to_string),
            gems,
            ..Default::default()
        }
    }

    fn build(sets: Vec<(Option<&str>, Vec<SkillGroup>)>) -> PobBuild {
        PobBuild {
            skill_sets: sets
                .into_iter()
                .map(|(title, groups)| SkillSet {
                    title: title.map(str::to_string),
                    groups,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn reads_a_rare() {
        let item = parse(BOW).unwrap();
        assert_eq!(item.class, "Bows");
        assert_eq!(item.rarity, "Rare");
        assert_eq!(item.title(), "Death Song, Short Bow");
        assert!(!item.is_gem());
    }

    #[test]
    fn reads_a_gem() {
        let item = parse(GEM).unwrap();
        assert!(item.is_gem());
        assert_eq!(item.name, "Added Fire Damage Support");
        assert_eq!(item.base, None);
    }

    #[test]
    fn a_magic_item_has_one_name_line() {
        let item = parse(
            "Item Class: Boots\nRarity: Magic\nRunner's Leather Boots of the Fox\n--------\n",
        )
        .unwrap();
        assert_eq!(item.title(), "Runner's Leather Boots of the Fox");
    }

    #[test]
    fn reads_the_level_requirement_not_the_gem_level() {
        let gem = "Item Class: Skill Gems\nRarity: Gem\nHerald of Ash\n--------\nLevel: 1\n--------\nRequirements:\nLevel: 16 (unmet)\nStr: 40\n--------\n";
        assert_eq!(parse(gem).unwrap().required_level, Some(16));
        let boots = "Item Class: Boots\nRarity: Normal\nIron Greaves\n--------\nArmour: 9\n--------\nRequirements:\nLevel: 5\n--------\nItem Level: 10";
        assert_eq!(parse(boots).unwrap().required_level, Some(5));
        assert_eq!(parse(BOW).unwrap().required_level, None);
    }

    #[test]
    fn other_text_is_not_an_item() {
        assert!(parse("hello").is_none());
        assert!(parse("").is_none());
        assert!(parse("Rarity: Rare\nDeath Song").is_none());
    }

    #[test]
    fn finds_the_groups_that_use_a_gem() {
        let b = build(vec![
            (
                Some("Act 1"),
                vec![group(
                    None,
                    vec![gem("Fireball", false), gem("Added Fire Damage", true)],
                )],
            ),
            (
                Some("Act 4"),
                vec![
                    group(
                        Some("Main"),
                        vec![
                            gem("Fireball", false),
                            gem("Added Fire Damage", true),
                            gem("Spell Echo", true),
                        ],
                    ),
                    group(None, vec![gem("Frostblink", false)]),
                ],
            ),
        ]);
        let card = gem_card(&b, None, "Added Fire Damage Support");
        assert_eq!(
            card.uses,
            vec![
                GemUse {
                    set: Some("Act 1".into()),
                    skill: "Fireball".into(),
                    links: vec![],
                },
                GemUse {
                    set: Some("Act 4".into()),
                    skill: "Main".into(),
                    links: vec!["Fireball".into(), "Spell Echo".into()],
                },
            ]
        );
    }

    #[test]
    fn the_same_setup_in_two_sets_shows_once() {
        let g = || {
            group(
                None,
                vec![gem("Fireball", false), gem("Arcane Surge", true)],
            )
        };
        let b = build(vec![(Some("Act 1"), vec![g()]), (Some("Act 2"), vec![g()])]);
        assert_eq!(gem_card(&b, None, "Fireball").uses.len(), 1);
    }

    #[test]
    fn disabled_gems_and_groups_dont_count() {
        let mut off = gem("Fireball", false);
        off.enabled = false;
        let mut hidden = group(None, vec![gem("Fireball", false)]);
        hidden.enabled = false;
        let b = build(vec![(None, vec![group(None, vec![off]), hidden])]);
        assert!(gem_card(&b, None, "Fireball").uses.is_empty());
    }

    #[test]
    fn says_where_the_campaign_gives_it() {
        let b = build(vec![(
            None,
            vec![group(None, vec![gem("Onslaught", true)])],
        )]);
        let plan = GemPlan {
            steps: vec![GemStep {
                gem_id: "G/Onslaught".into(),
                name: "Onslaught".into(),
                quest_id: "a1q2".into(),
                act: 1,
                action: GemAction::Buy,
                npc: "Nessa".into(),
                need_level: 1,
            }],
            ..Default::default()
        };
        let card = gem_card(&b, Some(&plan), "Onslaught Support");
        assert_eq!(card.get.as_deref(), Some("Sold by Nessa in act 1"));
        assert_eq!(card.uses[0].set, None);
    }
}
