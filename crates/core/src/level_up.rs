//! What a level-up makes usable: the build's gems whose time has come, and upgrades the player
//! checked earlier that needed a higher level.

use serde::Serialize;

use crate::gemplan::GemPlan;

/// An upgrade the item check found that the character couldn't wear yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Remembered {
    pub name: String,
    pub slot: String,
    pub level: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsableNow {
    pub level: u32,
    pub gems: Vec<String>,
    pub items: Vec<Remembered>,
    /// The level of the character the player imported, when it has fallen behind enough to say
    /// so. Said once every five levels, not on every level-up.
    pub stale: Option<u32>,
}

/// Levels behind before the card says the imported character is old.
const STALE_AFTER: u32 = 5;

/// Remembered items stay until they come due, and only the latest check of a slot counts.
pub fn remember(list: &mut Vec<Remembered>, item: Remembered) {
    list.retain(|r| r.slot != item.slot);
    list.push(item);
}

/// What levels `from + 1 ..= to` brought. Due items leave `remembered`. `imported` is the level
/// of the character imported from pathofexile.com, if there is one. None when nothing happened.
pub fn usable_now(
    plan: Option<&GemPlan>,
    remembered: &mut Vec<Remembered>,
    from: u32,
    to: u32,
    imported: Option<u32>,
) -> Option<UsableNow> {
    if to <= from {
        return None;
    }
    let reached = |level: u32| level > from && level <= to;
    let mut gems: Vec<String> = Vec::new();
    for step in plan.into_iter().flat_map(|p| &p.steps) {
        // Level 1 gems are in use from the start, not news on a level-up.
        if step.need_level > 1 && reached(step.need_level) && !gems.contains(&step.name) {
            gems.push(step.name.clone());
        }
    }
    let (items, rest): (Vec<_>, Vec<_>) = remembered.drain(..).partition(|r| reached(r.level));
    *remembered = rest;
    let stale = imported
        .filter(|&b| (from + 1..=to).any(|l| l >= b + STALE_AFTER && (l - b) % STALE_AFTER == 0));
    (!gems.is_empty() || !items.is_empty() || stale.is_some()).then_some(UsableNow {
        level: to,
        gems,
        items,
        stale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gemplan::{GemAction, GemStep};

    fn step(name: &str, need_level: u32) -> GemStep {
        GemStep {
            gem_id: name.into(),
            name: name.into(),
            quest_id: "q".into(),
            offer_id: "q".into(),
            act: 1,
            action: GemAction::Buy,
            npc: "Nessa".into(),
            need_level,
        }
    }

    fn item(slot: &str, level: u32) -> Remembered {
        Remembered {
            name: format!("{slot} item"),
            slot: slot.into(),
            level,
        }
    }

    fn plan() -> GemPlan {
        GemPlan {
            steps: vec![
                step("Fireball", 1),
                step("Herald of Ash", 16),
                step("Onslaught", 16),
                step("Flame Dash", 10),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn gems_the_build_starts_using_at_this_level() {
        let got = usable_now(Some(&plan()), &mut Vec::new(), 15, 16, None).unwrap();
        assert_eq!(got.level, 16);
        assert_eq!(got.gems, vec!["Herald of Ash", "Onslaught"]);
    }

    #[test]
    fn a_jump_of_levels_counts_every_level_in_between() {
        let got = usable_now(Some(&plan()), &mut Vec::new(), 9, 16, None).unwrap();
        assert_eq!(got.gems, vec!["Herald of Ash", "Onslaught", "Flame Dash"]);
    }

    #[test]
    fn nothing_new_is_none() {
        assert!(usable_now(Some(&plan()), &mut Vec::new(), 11, 12, None).is_none());
        assert!(usable_now(Some(&plan()), &mut Vec::new(), 16, 16, None).is_none());
        assert!(usable_now(None, &mut Vec::new(), 1, 2, None).is_none());
    }

    #[test]
    fn remembered_items_come_due_once() {
        let mut list = vec![item("Boots", 12), item("Helmet", 20)];
        let got = usable_now(None, &mut list, 11, 12, None).unwrap();
        assert_eq!(got.items, vec![item("Boots", 12)]);
        assert_eq!(list, vec![item("Helmet", 20)]);
        assert!(usable_now(None, &mut list, 12, 13, None).is_none());
    }

    #[test]
    fn a_new_check_of_a_slot_replaces_the_old_one() {
        let mut list = vec![item("Boots", 12)];
        remember(&mut list, item("Boots", 14));
        assert_eq!(list, vec![item("Boots", 14)]);
    }

    #[test]
    fn says_once_every_five_levels_that_the_imported_character_is_old() {
        let stale =
            |from, to| usable_now(None, &mut Vec::new(), from, to, Some(14)).and_then(|u| u.stale);
        assert_eq!(stale(15, 16), None);
        assert_eq!(stale(18, 19), Some(14));
        assert_eq!(stale(19, 20), None);
        assert_eq!(stale(23, 24), Some(14));
        // Two levels at once still count the one in between.
        assert_eq!(stale(17, 20), Some(14));
        assert!(usable_now(None, &mut Vec::new(), 18, 19, None).is_none());
    }
}
