//! A build's item as the overlay draws it: the game's tooltip, from PoB's item text.

use exile_pob::{ItemSet, ItemText, ModKind, PobBuild};
use serde::Serialize;

/// The slots in the order the game's inventory and PoB's Items tab go.
const SLOT_ORDER: [&str; 17] = [
    "Weapon 1",
    "Weapon 2",
    "Helmet",
    "Body Armour",
    "Gloves",
    "Boots",
    "Amulet",
    "Ring 1",
    "Ring 2",
    "Ring 3",
    "Belt",
    "Flask 1",
    "Flask 2",
    "Flask 3",
    "Flask 4",
    "Flask 5",
    "Weapon 1 Swap",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GearSlot {
    pub slot: String,
    pub item: ItemView,
}

/// The gear set a build wears now: the active one, else the first.
pub fn active_set(build: &PobBuild) -> Option<&ItemSet> {
    build
        .item_sets
        .iter()
        .find(|s| s.id.is_some() && s.id == build.active_item_set)
        .or_else(|| build.item_sets.first())
}

/// The gear that goes with skill set `set`: the gear set of the same title, as guides name a
/// stage's sets alike. A build that doesn't name its gear sets after stages, like an imported
/// character, shows the one it wears.
pub fn gear_for_set(build: &PobBuild, set: usize) -> Vec<GearSlot> {
    fn named(title: &Option<String>) -> Option<&str> {
        title.as_deref().map(str::trim).filter(|t| !t.is_empty())
    }
    let title = build.skill_sets.get(set).and_then(|s| named(&s.title));
    let stage_named = build.item_sets.iter().any(|g| {
        named(&g.title).is_some_and(|t| build.skill_sets.iter().any(|s| named(&s.title) == Some(t)))
    });
    let gear = if stage_named {
        build
            .item_sets
            .iter()
            .find(|g| title.is_some() && named(&g.title) == title)
    } else {
        active_set(build)
    };
    let mut slots: Vec<GearSlot> = gear
        .map(|g| g.slots.as_slice())
        .unwrap_or_default()
        .iter()
        .filter_map(|(slot, id)| {
            Some(GearSlot {
                slot: slot.clone(),
                item: ItemView::parse(&build.item(*id)?.text)?,
            })
        })
        .collect();
    slots.sort_by_key(|s| {
        SLOT_ORDER
            .iter()
            .position(|o| *o == s.slot)
            .unwrap_or(SLOT_ORDER.len())
    });
    slots
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
    /// "normal", "magic", "rare", "unique", "relic".
    pub rarity: String,
    pub name: String,
    pub base: Option<String>,
    pub quality: Option<u32>,
    pub sockets: Option<String>,
    pub level_req: Option<u32>,
    pub radius: Option<String>,
    pub corrupted: bool,
    pub mods: Vec<ModView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModView {
    pub text: String,
    pub kind: ModTone,
}

/// The colour group of a mod line in the game's tooltip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModTone {
    Enchant,
    Implicit,
    Explicit,
    Crafted,
    Fractured,
}

impl ItemView {
    pub fn parse(text: &str) -> Option<Self> {
        ItemText::parse(text).map(|t| Self::from(&t))
    }
}

impl From<&ItemText> for ItemView {
    fn from(t: &ItemText) -> Self {
        Self {
            rarity: t.rarity.to_lowercase(),
            name: t.name.clone(),
            base: t.base.clone(),
            quality: t.quality.filter(|q| *q > 0),
            sockets: t.sockets.clone(),
            level_req: t.level_req.filter(|l| *l > 0),
            radius: t.radius.clone(),
            corrupted: t.corrupted,
            mods: t
                .mods
                .iter()
                .map(|m| ModView {
                    text: m.text.clone(),
                    kind: match m.kind {
                        ModKind::Enchant => ModTone::Enchant,
                        ModKind::Implicit => ModTone::Implicit,
                        ModKind::Explicit => ModTone::Explicit,
                        ModKind::Crafted => ModTone::Crafted,
                        ModKind::Fractured => ModTone::Fractured,
                    },
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use exile_pob::{Item, SkillSet};

    use super::*;

    fn item(id: u32, name: &str) -> Item {
        Item {
            id,
            text: format!("Rarity: RARE\n{name}\nJudgement Staff\nImplicits: 0"),
        }
    }

    fn sets(titles: &[Option<&str>]) -> Vec<SkillSet> {
        titles
            .iter()
            .map(|t| SkillSet {
                title: t.map(String::from),
                ..SkillSet::default()
            })
            .collect()
    }

    #[test]
    fn a_stage_wears_the_gear_set_of_its_name_in_slot_order() {
        let build = PobBuild {
            skill_sets: sets(&[Some("Level 1-12"), Some("Midgame ")]),
            items: vec![item(1, "Ring Thing"), item(2, "Staff Thing")],
            item_sets: vec![
                ItemSet {
                    id: Some(1),
                    title: Some("Midgame".into()),
                    slots: vec![("Ring 1".into(), 1), ("Weapon 1".into(), 2)],
                },
                ItemSet {
                    id: Some(2),
                    title: Some("Level 1-12".into()),
                    slots: vec![],
                },
            ],
            ..PobBuild::default()
        };
        let gear = gear_for_set(&build, 1);
        let slots: Vec<&str> = gear.iter().map(|s| s.slot.as_str()).collect();
        assert_eq!(slots, vec!["Weapon 1", "Ring 1"]);
        assert!(gear_for_set(&build, 0).is_empty());
    }

    #[test]
    fn a_build_without_stage_named_sets_shows_what_it_wears() {
        let build = PobBuild {
            skill_sets: sets(&[None]),
            items: vec![item(1, "Worn")],
            active_item_set: Some(7),
            item_sets: vec![
                ItemSet {
                    id: Some(3),
                    title: None,
                    slots: vec![],
                },
                ItemSet {
                    id: Some(7),
                    title: None,
                    slots: vec![("Weapon 1".into(), 1)],
                },
            ],
            ..PobBuild::default()
        };
        let gear = gear_for_set(&build, 0);
        assert_eq!(gear.len(), 1);
        assert_eq!(gear[0].item.name, "Worn");
    }

    #[test]
    fn keeps_what_the_tooltip_shows() {
        let v = ItemView::parse(
            "Rarity: RARE\nNew Item\nFoul Staff\nQuality: 0\nLevelReq: 55\nImplicits: 1\n+22% Chance to Block\n{crafted}(80-99)% increased Physical Damage",
        )
        .unwrap();
        assert_eq!(v.rarity, "rare");
        assert_eq!(v.base.as_deref(), Some("Foul Staff"));
        assert_eq!(v.quality, None);
        assert_eq!(v.level_req, Some(55));
        assert_eq!(v.mods[0].kind, ModTone::Implicit);
        assert_eq!(v.mods[1].kind, ModTone::Crafted);
    }
}
