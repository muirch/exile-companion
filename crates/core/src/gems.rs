//! Gems and where the campaign hands them out: as a quest reward or from a vendor after a quest.
//!
//! Both come from exile-leveling's data (`crates/data/data/routes/exile-leveling/{gems,quests}.json`),
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
    /// Some quests hand out more than one reward, one per offer.
    pub offer_id: String,
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
            // Rewards that only the Chinese (Tencent) client offers.
            for (offer_id, offer) in quest
                .reward_offers
                .into_iter()
                .filter(|(id, _)| !id.ends_with("_tencent"))
            {
                for (gem_id, r) in offer.quest {
                    sources.push(GemSource {
                        gem_id,
                        quest_id: quest_id.clone(),
                        offer_id: offer_id.clone(),
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
                        offer_id: offer_id.clone(),
                        act,
                        kind: GemSourceKind::Vendor,
                        npc: r.npc.unwrap_or_default(),
                        classes: r.classes,
                    });
                }
            }
        }
        // Quest ids sort as text ("a10q1" before "a1q2"). Sort by act and quest number instead.
        sources.sort_by_key(|s| (s.act, quest_number(&s.quest_id)));
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

    /// Every place any class can get the gem, in campaign order.
    pub fn sources_of<'a>(&'a self, gem_id: &'a str) -> impl Iterator<Item = &'a GemSource> + 'a {
        self.sources.iter().filter(move |s| s.gem_id == gem_id)
    }

    /// Every place `class` can get the gem, in campaign order. An empty class list means anyone,
    /// like Siosa and Lilly Roth selling every gem.
    pub fn sources_for<'a>(
        &'a self,
        gem_id: &'a str,
        class: &'a str,
    ) -> impl Iterator<Item = &'a GemSource> + 'a {
        self.sources.iter().filter(move |s| {
            s.gem_id == gem_id && (s.classes.is_empty() || s.classes.iter().any(|c| c == class))
        })
    }
}

/// 4 for "a3q4", 11 for "a3q11"; side quests without a number go last in their act.
pub(crate) fn quest_number(quest_id: &str) -> u32 {
    quest_id
        .rsplit_once('q')
        .and_then(|(_, n)| {
            n.trim_end_matches(|c: char| !c.is_ascii_digit())
                .parse()
                .ok()
        })
        .unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEMS: &[u8] = include_bytes!("../../data/data/routes/exile-leveling/gems.json");
    const QUESTS: &[u8] = include_bytes!("../../data/data/routes/exile-leveling/quests.json");

    const CLASSES: [&str; 7] = [
        "Marauder", "Duelist", "Ranger", "Shadow", "Witch", "Templar", "Scion",
    ];

    fn table() -> GemTable {
        GemTable::from_json_bytes(GEMS, QUESTS).expect("committed gem data parses")
    }

    fn first_vendor<'a>(t: &'a GemTable, gem_id: &'a str, class: &'a str) -> (u32, &'a str) {
        t.sources_for(gem_id, class)
            .find(|s| s.kind == GemSourceKind::Vendor)
            .map(|s| (s.act, s.npc.as_str()))
            .unwrap()
    }

    // PoB writes the class as className. The data has to use the same names.
    #[test]
    fn the_data_names_the_seven_pob_classes() {
        let t = table();
        let named: std::collections::BTreeSet<&str> = t
            .sources
            .iter()
            .flat_map(|s| s.classes.iter().map(String::as_str))
            .collect();
        let mut want = CLASSES.to_vec();
        want.sort();
        assert_eq!(named.into_iter().collect::<Vec<_>>(), want);
    }

    #[test]
    fn each_class_has_its_own_act_1_vendor_list() {
        let t = table();
        let lists: std::collections::HashSet<Vec<&str>> = CLASSES
            .iter()
            .map(|class| {
                let mut ids: Vec<&str> = t
                    .gems
                    .keys()
                    .filter(|id| {
                        t.sources_for(id, class)
                            .any(|s| s.kind == GemSourceKind::Vendor && s.act == 1)
                    })
                    .map(String::as_str)
                    .collect();
                ids.sort();
                assert!(
                    ids.len() > 50,
                    "{class} buys only {} gems in act 1",
                    ids.len()
                );
                ids
            })
            .collect();
        assert_eq!(lists.len(), CLASSES.len());
    }

    #[test]
    fn class_locked_gems_wait_for_siosa() {
        let t = table();
        let fireball = "Metadata/Items/Gems/SkillGemFireball";
        assert_eq!(first_vendor(&t, fireball, "Witch"), (1, "Nessa"));
        assert_eq!(first_vendor(&t, fireball, "Ranger"), (3, "Siosa"));
        let caustic = "Metadata/Items/Gems/SkillGemPoisonArrow";
        assert_eq!(first_vendor(&t, caustic, "Ranger"), (1, "Nessa"));
        assert_eq!(first_vendor(&t, caustic, "Templar"), (3, "Siosa"));
        // Frostblink is in every class's first shop.
        for class in CLASSES {
            assert_eq!(
                first_vendor(&t, "Metadata/Items/Gems/SkillGemFrostblink", class),
                (1, "Nessa"),
                "{class}"
            );
        }
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
    fn vendors_without_a_class_list_sell_to_everyone() {
        let t = table();
        let npcs: Vec<_> = t
            .sources_for("Metadata/Items/Gems/SkillGemClarity", "Ranger")
            .filter(|s| s.kind == GemSourceKind::Vendor)
            .map(|s| s.npc.as_str())
            .collect();
        assert_eq!(npcs, ["Nessa", "Siosa", "Lilly Roth"]);
    }

    #[test]
    fn a_quest_with_two_offers_keeps_them_apart() {
        let t = table();
        let offers: std::collections::BTreeSet<_> = t
            .sources_for("Metadata/Items/Gems/SkillGemFlameDash", "Witch")
            .chain(t.sources_for("Metadata/Items/Gems/SupportGemLifetap", "Templar"))
            .filter(|s| s.quest_id == "a1q2" && s.kind == GemSourceKind::QuestReward)
            .map(|s| s.offer_id.as_str())
            .collect();
        assert_eq!(offers.into_iter().collect::<Vec<_>>(), ["a1q2", "a1q2b"]);
    }

    #[test]
    fn tencent_only_rewards_are_left_out() {
        let t = table();
        assert!(t.sources.iter().all(|s| !s.offer_id.ends_with("_tencent")));
        assert!(t
            .sources_for("Metadata/Items/Gems/SupportGemArcaneSurge", "Witch")
            .all(|s| s.quest_id != "a2q9"));
    }

    #[test]
    fn other_classes_dont_see_class_locked_rewards() {
        let t = table();
        assert!(t
            .sources_for("Metadata/Items/Gems/SkillGemRaiseZombie", "Marauder")
            .all(|s| s.kind == GemSourceKind::Vendor));
    }
}
