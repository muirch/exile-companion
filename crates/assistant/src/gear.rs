//! "Is this item an upgrade" without a model: the MCP server's compare_item answer for the best
//! slot, cut down to what the overlay shows in a glance.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Upgrade,
    Downgrade,
    /// Better at one of damage and defence, worse at the other.
    Mixed,
    Same,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resist {
    /// "Fire", "Cold", "Lightning" or "Chaos".
    pub element: String,
    pub change: f64,
    /// The resistance after the swap, capped as the character has it.
    pub after: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GearCard {
    pub slot: String,
    /// The equipped item it would replace.
    pub replaces: Option<String>,
    pub verdict: Verdict,
    pub dps_pct: f64,
    pub ehp_pct: f64,
    pub life: f64,
    pub resists: Vec<Resist>,
    /// Some mods or effects PoB couldn't calculate, so the numbers leave them out.
    pub partly: bool,
}

// Changes smaller than this are noise in a glance.
const NOTICE_PCT: f64 = 1.0;

const RESISTS: [(&str, &str); 4] = [
    ("FireResist", "Fire"),
    ("ColdResist", "Cold"),
    ("LightningResist", "Lightning"),
    ("ChaosResist", "Chaos"),
];

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

pub fn verdict(dps_pct: f64, ehp_pct: f64) -> Verdict {
    let up = dps_pct >= NOTICE_PCT || ehp_pct >= NOTICE_PCT;
    let down = dps_pct <= -NOTICE_PCT || ehp_pct <= -NOTICE_PCT;
    match (up, down) {
        (true, true) => Verdict::Mixed,
        (true, false) => Verdict::Upgrade,
        (false, true) => Verdict::Downgrade,
        (false, false) => Verdict::Same,
    }
}

/// None when the item fits no slot of the build.
pub fn from_compare_item(v: &Value) -> Option<GearCard> {
    let best = v["slots"].as_array()?.first()?;
    let dps_pct = num(&best["dps_pct"]);
    let ehp_pct = num(&best["ehp_pct"]);
    let resists = best["changes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let stat = c["stat"].as_str()?;
            let (_, element) = RESISTS.iter().find(|(s, _)| *s == stat)?;
            let after = num(&c["after"]);
            Some(Resist {
                element: (*element).to_string(),
                change: after - num(&c["before"]),
                after,
            })
        })
        .collect();
    let not_calculated = v
        .pointer("/item/not_calculated")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty());
    Some(GearCard {
        slot: best["slot"].as_str()?.to_string(),
        replaces: best["replaces"].as_str().map(str::to_string),
        verdict: verdict(dps_pct, ehp_pct),
        dps_pct,
        ehp_pct,
        life: num(&best["life"]),
        resists,
        partly: not_calculated || v["tree_not_rebuilt"] == true,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_the_best_slot() {
        let v = json!({
            "item": {"name": "Death Song", "not_calculated": []},
            "slots": [
                {"slot": "Ring 1", "replaces": "Iron Ring", "dps_pct": 4.2, "ehp_pct": 0.3, "life": 0,
                 "changes": [
                    {"stat": "FireResist", "label": "Fire Resistance", "before": 40, "after": 55},
                    {"stat": "Life", "label": "Life", "before": 500, "after": 500.5}
                 ]},
                {"slot": "Ring 2", "dps_pct": 1, "ehp_pct": 0}
            ]
        });
        let card = from_compare_item(&v).unwrap();
        assert_eq!(card.slot, "Ring 1");
        assert_eq!(card.replaces.as_deref(), Some("Iron Ring"));
        assert_eq!(card.verdict, Verdict::Upgrade);
        assert_eq!(
            card.resists,
            vec![Resist {
                element: "Fire".into(),
                change: 15.0,
                after: 55.0
            }]
        );
        assert!(!card.partly);
    }

    #[test]
    fn an_empty_slot_replaces_nothing() {
        let v = json!({"slots": [{"slot": "Belt", "dps_pct": 0, "ehp_pct": 6}]});
        let card = from_compare_item(&v).unwrap();
        assert_eq!(card.replaces, None);
        assert_eq!(card.verdict, Verdict::Upgrade);
    }

    #[test]
    fn an_item_with_no_slot_has_no_card() {
        assert!(from_compare_item(&json!({"slots": []})).is_none());
        assert!(from_compare_item(&json!({})).is_none());
    }

    #[test]
    fn what_pob_cant_calculate_marks_the_card() {
        let v = json!({
            "item": {"not_calculated": ["Grants a mechanic from a future league"]},
            "slots": [{"slot": "Helmet", "dps_pct": 0, "ehp_pct": 0}]
        });
        assert!(from_compare_item(&v).unwrap().partly);
        let jewel = json!({"slots": [{"slot": "Jewel 1"}], "tree_not_rebuilt": true});
        assert!(from_compare_item(&jewel).unwrap().partly);
    }

    #[test]
    fn verdicts() {
        assert_eq!(verdict(5.0, -3.0), Verdict::Mixed);
        assert_eq!(verdict(-2.0, 0.0), Verdict::Downgrade);
        assert_eq!(verdict(0.4, -0.4), Verdict::Same);
        assert_eq!(verdict(0.0, 1.0), Verdict::Upgrade);
    }
}
