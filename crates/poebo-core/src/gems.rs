//! Gems and where the campaign hands them out: as a quest reward or from a vendor after a quest.
//!
//! Both come from exile-leveling's data (`data-pack/routes/exile-leveling/{gems,quests}.json`),
//! because the game files don't say what each vendor sells after a quest.

use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;

use crate::error::DataError;

/// One gem. `id` is the base item metadata path, the same id Path of Building writes as `gemId`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GemInfo {
    pub id: String,
    pub name: String,
    pub required_level: u32,
    pub is_support: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GemSourceKind {
    /// Picked as the quest reward: one gem per quest.
    QuestReward,
    /// Bought from `npc` once the quest is done.
    Vendor,
}

/// One way to get a gem in the campaign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GemSource {
    pub gem_id: String,
    pub quest_id: String,
    pub act: u32,
    pub kind: GemSourceKind,
    pub npc: String,
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GemTable {
    gems: HashMap<String, GemInfo>,
    sources: Vec<GemSource>,
}

#[derive(Deserialize)]
struct RawGem {
    id: String,
    name: String,
    required_level: u32,
    is_support: bool,
}

#[derive(Deserialize)]
struct RawQuest {
    act: String,
    reward_offers: BTreeMap<String, RawOffer>,
}

#[derive(Deserialize)]
struct RawOffer {
    #[serde(default)]
    quest_npc: String,
    #[serde(default)]
    quest: BTreeMap<String, RawReward>,
    #[serde(default)]
    vendor: BTreeMap<String, RawReward>,
}

#[derive(Deserialize)]
struct RawReward {
    classes: Vec<String>,
    #[serde(default)]
    npc: Option<String>,
}

impl GemTable {
    pub fn from_json_bytes(gems: &[u8], quests: &[u8]) -> Result<Self, DataError> {
        let raw_gems: HashMap<String, RawGem> = serde_json::from_slice(gems)?;
        let raw_quests: BTreeMap<String, RawQuest> = serde_json::from_slice(quests)?;

        let gems = raw_gems
            .into_values()
            .map(|g| {
                let info = GemInfo {
                    id: g.id,
                    name: g.name,
                    required_level: g.required_level,
                    is_support: g.is_support,
                };
                (info.id.clone(), info)
            })
            .collect();

        let mut sources = Vec::new();
        for (quest_id, quest) in raw_quests {
            let act = quest.act.parse().unwrap_or(0);
            for offer in quest.reward_offers.into_values() {
                for (gem_id, r) in offer.quest {
                    sources.push(GemSource {
                        gem_id,
                        quest_id: quest_id.clone(),
                        act,
                        kind: GemSourceKind::QuestReward,
                        npc: r.npc.unwrap_or_else(|| offer.quest_npc.clone()),
                        classes: r.classes,
                    });
                }
                for (gem_id, r) in offer.vendor {
                    sources.push(GemSource {
                        gem_id,
                        quest_id: quest_id.clone(),
                        act,
                        kind: GemSourceKind::Vendor,
                        npc: r.npc.unwrap_or_default(),
                        classes: r.classes,
                    });
                }
            }
        }
        Ok(Self { gems, sources })
    }

    pub fn gem(&self, id: &str) -> Option<&GemInfo> {
        self.gems.get(id)
    }

    pub fn len(&self) -> usize {
        self.gems.len()
    }

    pub fn is_empty(&self) -> bool {
        self.gems.is_empty()
    }

    /// Every place `class` can get the gem, in campaign order.
    pub fn sources_for<'a>(
        &'a self,
        gem_id: &'a str,
        class: &'a str,
    ) -> impl Iterator<Item = &'a GemSource> + 'a {
        self.sources
            .iter()
            .filter(move |s| s.gem_id == gem_id && s.classes.iter().any(|c| c == class))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEMS: &[u8] = include_bytes!("../../../data-pack/routes/exile-leveling/gems.json");
    const QUESTS: &[u8] = include_bytes!("../../../data-pack/routes/exile-leveling/quests.json");

    fn table() -> GemTable {
        GemTable::from_json_bytes(GEMS, QUESTS).expect("committed gem data parses")
    }

    #[test]
    fn loads_the_committed_gem_list() {
        let t = table();
        assert!(t.len() > 500, "only {} gems", t.len());
        let fireball = t.gem("Metadata/Items/Gems/SkillGemFireball").unwrap();
        assert_eq!(fireball.name, "Fireball");
        assert_eq!(fireball.required_level, 1);
        assert!(!fireball.is_support);
    }

    #[test]
    fn witch_gets_raise_zombie_from_the_first_quest() {
        let t = table();
        let src: Vec<_> = t
            .sources_for("Metadata/Items/Gems/SkillGemRaiseZombie", "Witch")
            .filter(|s| s.kind == GemSourceKind::QuestReward)
            .collect();
        assert!(src
            .iter()
            .any(|s| s.quest_id == "a1q1" && s.npc == "Tarkleigh" && s.act == 1));
    }

    #[test]
    fn vendor_sources_name_the_npc() {
        let t = table();
        let src: Vec<_> = t
            .sources_for("Metadata/Items/Gems/SupportGemArcaneSurge", "Witch")
            .filter(|s| s.kind == GemSourceKind::Vendor)
            .collect();
        assert!(src.iter().any(|s| s.quest_id == "a1q1" && s.npc == "Nessa"));
    }

    #[test]
    fn other_classes_dont_see_class_locked_rewards() {
        let t = table();
        assert!(t
            .sources_for("Metadata/Items/Gems/SkillGemRaiseZombie", "Marauder")
            .all(|s| s.kind == GemSourceKind::Vendor));
    }
}
