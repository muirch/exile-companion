//! Search strings for the game's own item search, so a vendor window highlights the gems the
//! route says to buy. The game draws the frames, we only build the regex.

use std::time::{Duration, Instant};

use crate::gemplan::{GemAction, GemPlan};
use crate::route::{Part, RouteView};

/// The game's search field takes 250 characters.
const MAX_LEN: usize = 250;

/// Gems to buy where the player is now: the buys the overlay lists for this area, else every buy
/// of the plan in the current act (the player came back to town after the route moved on).
pub fn vendor_gems(view: &RouteView, plan: &GemPlan) -> Vec<String> {
    let mut names = Vec::new();
    for part in view.here.iter().flat_map(|s| &s.parts) {
        if let Part::RewardVendor { item, .. } = part {
            push_unique(&mut names, item);
        }
    }
    if names.is_empty() {
        if let Some(act) = section_act(&view.section) {
            for step in &plan.steps {
                if step.action == GemAction::Buy && step.act == act {
                    push_unique(&mut names, &step.name);
                }
            }
        }
    }
    names
}

fn push_unique(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|n| n == name) {
        names.push(name.to_string());
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
// shrinks to `.*t`, the trick Exile-UI uses to fit more gems into one string.
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
    use crate::gemplan::GemStep;
    use crate::route::Step;

    fn view(section: &str, parts: Vec<Part>) -> RouteView {
        RouteView {
            section: section.into(),
            edge: 3,
            edge_count: 10,
            section_edge: 3,
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

    fn vendor(item: &str) -> Part {
        Part::RewardVendor {
            item: item.into(),
            cost: None,
        }
    }

    fn buy(name: &str, act: u32) -> GemStep {
        GemStep {
            gem_id: name.into(),
            name: name.into(),
            quest_id: "q".into(),
            act,
            action: GemAction::Buy,
            npc: "Nessa".into(),
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
    fn takes_the_buys_listed_here() {
        let v = view(
            "Act 1",
            vec![
                Part::Text {
                    text: "Buy ".into(),
                },
                vendor("Frostblink"),
                vendor("Frostblink"),
                Part::RewardQuest {
                    item: "Fireball".into(),
                },
                vendor("Arcane Surge Support"),
            ],
        );
        let plan = GemPlan {
            steps: vec![buy("Clarity", 1)],
            ..Default::default()
        };
        assert_eq!(
            vendor_gems(&v, &plan),
            vec!["Frostblink", "Arcane Surge Support"]
        );
    }

    #[test]
    fn falls_back_to_the_acts_buys() {
        let v = view("Act 2", vec![Part::Text { text: "Go".into() }]);
        let plan = GemPlan {
            steps: vec![
                buy("Clarity", 1),
                buy("Herald of Ash", 2),
                buy("Onslaught Support", 2),
            ],
            ..Default::default()
        };
        assert_eq!(
            vendor_gems(&v, &plan),
            vec!["Herald of Ash", "Onslaught Support"]
        );
    }

    #[test]
    fn quest_rewards_are_not_bought() {
        let v = view("Act 2", vec![Part::Text { text: "Go".into() }]);
        let mut take = buy("Fireball", 2);
        take.action = GemAction::Take;
        let plan = GemPlan {
            steps: vec![take],
            ..Default::default()
        };
        assert!(vendor_gems(&v, &plan).is_empty());
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
