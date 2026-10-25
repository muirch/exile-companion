//! Search strings for the game's own item search, so a vendor window highlights the gems the
//! route says to buy. The game draws the frames, we only build the regex.

use std::time::{Duration, Instant};

use crate::gemplan::{GemAction, GemPlan, GemStep};
use crate::gems::GemTable;
use crate::route::{Part, RouteView};

/// The game's search field takes 250 characters.
const MAX_LEN: usize = 250;

/// Gems to buy where the player is now, by their names in the game: the buys after the quests
/// handed in here (and the mules' on the first step), else every buy of the plan in the current
/// act (the player came back to town after the route moved on).
pub fn vendor_gems(view: &RouteView, plan: &GemPlan, gems: &GemTable) -> Vec<String> {
    let quests: Vec<&str> = view
        .here
        .iter()
        .flat_map(|s| &s.parts)
        .filter_map(|p| match p {
            Part::Quest { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    let game_name = |name: &str| {
        gems.by_name(name)
            .map_or_else(|| name.to_string(), |g| g.name.clone())
    };
    let step_name = |s: &GemStep| {
        gems.gem(&s.gem_id)
            .map_or_else(|| game_name(&s.name), |g| g.name.clone())
    };
    let mut names = Vec::new();
    if view.edge == 0 {
        for gem in plan.mules.iter().flat_map(|m| &m.buy) {
            push_unique(&mut names, game_name(&gem.name));
        }
    }
    for step in &plan.steps {
        if step.action == GemAction::Buy && quests.contains(&step.quest_id.as_str()) {
            push_unique(&mut names, step_name(step));
        }
    }
    if names.is_empty() {
        if let Some(act) = section_act(&view.section) {
            for step in &plan.steps {
                if step.action == GemAction::Buy && step.act == act {
                    push_unique(&mut names, step_name(step));
                }
            }
        }
    }
    names
}

fn push_unique(names: &mut Vec<String>, name: String) {
    if !names.contains(&name) {
        names.push(name);
    }
}

fn section_act(section: &str) -> Option<u32> {
    section.strip_prefix("Act ")?.trim().parse().ok()
}

/// One or more strings like `^(frostblink|added.fire.damage.*t)$`, each short enough for the
/// search field. Anchored, because the search runs per line and "Clarity" would also hit a
/// description that names it.
pub fn search_strings<S: AsRef<str>>(names: &[S]) -> Vec<String> {
    let budget = MAX_LEN - "^()$".len();
    let mut out = Vec::new();
    let mut current = String::new();
    for name in names {
        let pattern = pattern(name.as_ref());
        if !current.is_empty() && current.len() + 1 + pattern.len() > budget {
            out.push(wrap(&current));
            current.clear();
        }
        if !current.is_empty() {
            current.push('|');
        }
        current.push_str(&pattern);
    }
    if !current.is_empty() {
        out.push(wrap(&current));
    }
    out
}

fn wrap(alternatives: &str) -> String {
    format!("^({alternatives})$")
}

// Spaces and punctuation become `.`, so the string needs no quotes and no escapes. " Support"
// shrinks to `.*t` so more gems fit into one string.
fn pattern(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    let (base, support) = match lower.strip_suffix(" support") {
        Some(base) => (base, true),
        None => (lower.as_str(), false),
    };
    let mut p: String = base
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '.' })
        .collect();
    if support {
        p.push_str(".*t");
    }
    p
}

/// Which string the next key press types. Pressing again soon after types the next one, so a
/// list that needs two strings is two presses.
#[derive(Debug, Default)]
pub struct SearchCycle {
    strings: Vec<String>,
    next: usize,
    last: Option<Instant>,
}

/// One string to type, and its place among the strings for this vendor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPress {
    pub text: String,
    pub index: usize,
    pub count: usize,
}

impl SearchCycle {
    const AGAIN_WITHIN: Duration = Duration::from_secs(30);

    pub fn press(&mut self, strings: Vec<String>, now: Instant) -> Option<SearchPress> {
        if strings.is_empty() {
            *self = Self::default();
            return None;
        }
        let again = self
            .last
            .is_some_and(|t| now.saturating_duration_since(t) < Self::AGAIN_WITHIN);
        if strings != self.strings || !again {
            self.strings = strings;
            self.next = 0;
        }
        let index = self.next % self.strings.len();
        self.next = index + 1;
        self.last = Some(now);
        Some(SearchPress {
            text: self.strings[index].clone(),
            index,
            count: self.strings.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gemplan::{Mule, MuleGem};
    use crate::route::Step;

    const GEMS: &str = r#"{
      "G/Frostblink": {"id": "G/Frostblink", "name": "Frostblink", "required_level": 4, "is_support": false},
      "G/ArcaneSurge": {"id": "G/ArcaneSurge", "name": "Arcane Surge Support", "required_level": 1, "is_support": true},
      "G/Clarity": {"id": "G/Clarity", "name": "Clarity", "required_level": 8, "is_support": false},
      "G/HeraldAsh": {"id": "G/HeraldAsh", "name": "Herald of Ash", "required_level": 16, "is_support": false},
      "G/Onslaught": {"id": "G/Onslaught", "name": "Onslaught Support", "required_level": 1, "is_support": true}
    }"#;

    fn gems() -> GemTable {
        GemTable::from_json_bytes(GEMS.as_bytes(), b"{}").unwrap()
    }

    fn view(section: &str, edge: u32, parts: Vec<Part>) -> RouteView {
        RouteView {
            section: section.into(),
            edge,
            edge_count: 10,
            section_edge: edge,
            section_edge_count: 5,
            finished_section: None,
            area_id: "1_1_town".into(),
            here: vec![Step {
                parts,
                sub_steps: Vec::new(),
                edge: None,
            }],
            then: None,
        }
    }

    fn quest(id: &str) -> Part {
        Part::Quest {
            id: id.into(),
            name: id.into(),
        }
    }

    // PoB's names: supports without " Support".
    fn step(id: &str, name: &str, quest: &str, act: u32, action: GemAction) -> GemStep {
        GemStep {
            gem_id: id.into(),
            name: name.into(),
            quest_id: quest.into(),
            act,
            action,
            npc: "Nessa".into(),
            need_level: 1,
        }
    }

    #[test]
    fn pattern_shortens_supports_and_drops_spaces() {
        assert_eq!(pattern("Added Fire Damage Support"), "added.fire.damage.*t");
        assert_eq!(pattern("Frostblink"), "frostblink");
        assert_eq!(pattern("Summon Raging Spirit"), "summon.raging.spirit");
    }

    #[test]
    fn punctuation_never_reaches_the_regex() {
        let p = pattern("Elemental Damage with Attacks (Test) Support");
        assert!(p
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '*'));
    }

    #[test]
    fn one_string_for_a_short_list() {
        assert_eq!(
            search_strings(&["Frostblink", "Onslaught Support"]),
            vec!["^(frostblink|onslaught.*t)$".to_string()]
        );
    }

    #[test]
    fn long_lists_split_under_the_field_limit() {
        let names: Vec<String> = (0..40).map(|i| format!("Gem Number {i} Support")).collect();
        let strings = search_strings(&names);
        assert!(strings.len() > 1);
        assert!(strings.iter().all(|s| s.len() <= MAX_LEN));
        let joined: usize = strings.iter().map(|s| s.matches('|').count() + 1).sum();
        assert_eq!(joined, 40);
    }

    #[test]
    fn no_names_no_strings() {
        assert!(search_strings::<&str>(&[]).is_empty());
    }

    #[test]
    fn buys_after_the_quests_handed_in_here_by_game_name() {
        let v = view("Act 1", 3, vec![quest("a1q1")]);
        let plan = GemPlan {
            steps: vec![
                step("G/Frostblink", "Frostblink", "a1q1", 1, GemAction::Buy),
                step("G/ArcaneSurge", "Arcane Surge", "a1q1", 1, GemAction::Buy),
                step("G/Clarity", "Clarity", "a1q5", 1, GemAction::Buy),
            ],
            ..Default::default()
        };
        assert_eq!(
            vendor_gems(&v, &plan, &gems()),
            vec!["Frostblink", "Arcane Surge Support"]
        );
    }

    #[test]
    fn falls_back_to_the_acts_buys() {
        let v = view("Act 2", 3, vec![Part::Text { text: "Go".into() }]);
        let plan = GemPlan {
            steps: vec![
                step("G/Clarity", "Clarity", "a1q1", 1, GemAction::Buy),
                step("G/HeraldAsh", "Herald of Ash", "a2q1", 2, GemAction::Buy),
                step("G/Onslaught", "Onslaught", "a2q2", 2, GemAction::Buy),
            ],
            ..Default::default()
        };
        assert_eq!(
            vendor_gems(&v, &plan, &gems()),
            vec!["Herald of Ash", "Onslaught Support"]
        );
    }

    #[test]
    fn quest_rewards_are_not_bought() {
        let v = view("Act 2", 3, vec![quest("a2q1")]);
        let plan = GemPlan {
            steps: vec![step(
                "G/HeraldAsh",
                "Herald of Ash",
                "a2q1",
                2,
                GemAction::Take,
            )],
            ..Default::default()
        };
        assert!(vendor_gems(&v, &plan, &gems()).is_empty());
    }

    #[test]
    fn mules_buy_on_the_first_step() {
        let v = view(
            "Act 1",
            0,
            vec![Part::Text {
                text: "Start".into(),
            }],
        );
        let plan = GemPlan {
            mules: vec![Mule {
                class: "Shadow".into(),
                take: None,
                buy: vec![MuleGem {
                    name: "Onslaught".into(),
                    npc: "Nessa".into(),
                    own_act: None,
                }],
            }],
            ..Default::default()
        };
        assert_eq!(vendor_gems(&v, &plan, &gems()), vec!["Onslaught Support"]);
    }

    #[test]
    fn pressing_again_types_the_next_string() {
        let mut cycle = SearchCycle::default();
        let t = Instant::now();
        let strings = vec!["a".to_string(), "b".to_string()];
        let first = cycle.press(strings.clone(), t).unwrap();
        assert_eq!((first.text.as_str(), first.index, first.count), ("a", 0, 2));
        let second = cycle
            .press(strings.clone(), t + Duration::from_secs(2))
            .unwrap();
        assert_eq!(second.text, "b");
        let third = cycle.press(strings, t + Duration::from_secs(4)).unwrap();
        assert_eq!(third.text, "a");
    }

    #[test]
    fn a_later_press_or_another_vendor_starts_over() {
        let mut cycle = SearchCycle::default();
        let t = Instant::now();
        let strings = vec!["a".to_string(), "b".to_string()];
        cycle.press(strings.clone(), t);
        let later = cycle.press(strings, t + Duration::from_secs(60)).unwrap();
        assert_eq!(later.text, "a");
        let other = cycle
            .press(vec!["c".to_string()], t + Duration::from_secs(61))
            .unwrap();
        assert_eq!(other.text, "c");
    }

    #[test]
    fn nothing_to_buy_types_nothing() {
        let mut cycle = SearchCycle::default();
        assert!(cycle.press(Vec::new(), Instant::now()).is_none());
    }
}
