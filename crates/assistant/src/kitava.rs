//! Resistances after Kitava, measured with the build's PoB before the fight. Kitava takes 30% of
//! every resistance in act 5 and 30% more in act 10.

use serde::Serialize;
use serde_json::{json, Value};

/// What the game caps elemental resistances at without anything that raises it.
pub const CAP: f64 = 75.0;

const ELEMENTS: [&str; 3] = ["Fire", "Cold", "Lightning"];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Short {
    pub element: String,
    /// The resistance once Kitava's penalty applies.
    pub after: f64,
}

/// The what_if changes that set PoB's resistance penalty. Its Configuration calls it
/// resistancePenalty: 0, -30 or -60.
pub fn penalty_change(penalty: i32) -> Value {
    json!({ "changes": [{ "config": "resistancePenalty", "value": penalty }] })
}

/// Elemental resistances below the cap at the penalty, from the defences answer (the build as it
/// is configured) and the what_if answer with the penalty set (only what changed).
pub fn short_resists(defences: &Value, what_if: &Value) -> Vec<Short> {
    let changed = |stat: &str| {
        what_if["changes"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["stat"] == stat)
            .and_then(|c| c["after"].as_f64())
    };
    let current = |element: &str| {
        defences["damage_types"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|t| t["type"] == element)
            .and_then(|t| t["resist"].as_f64())
    };
    ELEMENTS
        .iter()
        .filter_map(|&e| {
            let after = changed(&format!("{e}Resist")).or_else(|| current(e))?;
            (after < CAP).then(|| Short {
                element: e.to_string(),
                after,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defences(fire: f64, cold: f64, lightning: f64) -> Value {
        json!({ "damage_types": [
            {"type": "Physical", "resist": 0},
            {"type": "Fire", "resist": fire},
            {"type": "Cold", "resist": cold},
            {"type": "Lightning", "resist": lightning},
            {"type": "Chaos", "resist": -60}
        ]})
    }

    #[test]
    fn takes_the_penalty_from_what_changed() {
        let what_if = json!({ "changes": [
            {"stat": "FireResist", "before": 75, "after": 60},
            {"stat": "ColdResist", "before": 75, "after": 75}
        ]});
        let short = short_resists(&defences(75.0, 75.0, 40.0), &what_if);
        assert_eq!(
            short,
            vec![
                Short {
                    element: "Fire".into(),
                    after: 60.0
                },
                Short {
                    element: "Lightning".into(),
                    after: 40.0
                },
            ]
        );
    }

    #[test]
    fn capped_after_the_penalty_says_nothing() {
        let short = short_resists(&defences(75.0, 75.0, 75.0), &json!({ "changes": [] }));
        assert!(short.is_empty());
    }

    #[test]
    fn chaos_is_left_out() {
        assert!(short_resists(&defences(75.0, 75.0, 75.0), &json!({})).is_empty());
    }

    #[test]
    fn the_change_sets_the_config() {
        assert_eq!(
            penalty_change(-30)["changes"][0],
            json!({ "config": "resistancePenalty", "value": -30 })
        );
    }
}
