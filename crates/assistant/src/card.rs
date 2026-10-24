//! "What next" without a model: the MCP server's what_next answer, cut down to the few changes
//! that bring the build closest to the followed guide's stage.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Gem,
    Passive,
    Gear,
    Ascendancy,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub kind: Kind,
    pub name: String,
    /// The gear slot, for gear.
    pub slot: Option<String>,
    /// Passive points to spend, the path included.
    pub points: Option<u64>,
    pub dps_pct: f64,
    pub ehp_pct: f64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NextCard {
    /// Why there are no steps, in the player's words.
    pub missing: Option<String>,
    /// The followed guide's tree the build is compared with.
    pub stage: Option<String>,
    /// A later tree of the guide the character's level has reached.
    pub later_stage: Option<String>,
    pub steps: Vec<Step>,
}

const PER_KIND: usize = 3;

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

fn worth(s: &Step) -> f64 {
    let gain = s.dps_pct + s.ehp_pct;
    match s.points {
        Some(p) if p > 0 => gain / p as f64,
        _ => gain,
    }
}

pub fn from_what_next(v: &Value) -> NextCard {
    if v["guide"].is_string() {
        return NextCard {
            missing: Some("Follow a guide to see what to do next. Paste its Path of Building link in the chat and ask the assistant to follow it.".into()),
            ..Default::default()
        };
    }
    if v["build"].is_string() {
        return NextCard {
            missing: Some("Import your build in the settings, or ask the assistant to make the guide your build.".into()),
            ..Default::default()
        };
    }
    let diff = &v["guide_diff"];
    let trees = diff.pointer("/reference/trees").and_then(Value::as_array);
    let stage = trees
        .and_then(|t| t.iter().find(|t| t["active"] == true))
        .and_then(|t| t["title"].as_str())
        .map(str::to_string);
    let later_stage = v
        .pointer("/later_stage/tree/title")
        .and_then(Value::as_str)
        .map(str::to_string);

    let mut gems: Vec<Step> = diff
        .pointer("/supports/missing")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|g| {
            Some(Step {
                kind: Kind::Gem,
                name: g["gem"].as_str()?.to_string(),
                slot: None,
                points: None,
                dps_pct: num(&g["dps_pct"]),
                ehp_pct: num(&g["ehp_pct"]),
            })
        })
        .collect();

    // The guide's missing notables and the best next ones overlap. Each shows once.
    let mut passives: Vec<Step> = Vec::new();
    let notables = diff
        .pointer("/passives/missing")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(v["next_passives"].as_array().into_iter().flatten());
    for p in notables {
        let Some(name) = p.pointer("/node/name").and_then(Value::as_str) else {
            continue;
        };
        if passives.iter().any(|s| s.name == name) {
            continue;
        }
        passives.push(Step {
            kind: Kind::Passive,
            name: name.to_string(),
            slot: None,
            points: p["points"].as_u64(),
            dps_pct: num(&p["dps_pct"]),
            ehp_pct: num(&p["ehp_pct"]),
        });
    }

    let mut gear: Vec<Step> = diff["gear"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| {
            let item = &g["reference_item"];
            let name = item["name"].as_str().or(item["base"].as_str())?.to_string();
            Some(Step {
                kind: Kind::Gear,
                name,
                slot: g["slot"].as_str().map(str::to_string),
                points: None,
                dps_pct: num(&g["dps_pct"]),
                ehp_pct: num(&g["ehp_pct"]),
            })
        })
        .collect();

    let ascendancy: Vec<Step> = diff
        .pointer("/ascendancy/missing")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let name = a
                .as_str()
                .or(a.pointer("/node/name").and_then(Value::as_str))?;
            Some(Step {
                kind: Kind::Ascendancy,
                name: name.to_string(),
                slot: None,
                points: None,
                dps_pct: 0.0,
                ehp_pct: 0.0,
            })
        })
        .collect();

    let by_worth = |a: &Step, b: &Step| worth(b).total_cmp(&worth(a));
    gems.sort_by(by_worth);
    passives.sort_by(by_worth);
    gear.sort_by(by_worth);
    // Gems first: a support from a vendor is the quickest change, gear the slowest.
    let steps: Vec<Step> = [gems, passives, gear, ascendancy]
        .into_iter()
        .flat_map(|k| k.into_iter().take(PER_KIND))
        .collect();
    NextCard {
        missing: steps
            .is_empty()
            .then(|| "Your build matches the guide's stage.".to_string()),
        stage,
        later_stage,
        steps,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn answer() -> Value {
        json!({
            "status": {"game": {"level": 12}},
            "guide_diff": {
                "ascendancy": {"extra": [], "missing": []},
                "passives": {"small_passives_missing": 7, "extra": [], "missing": [
                    {"points": 8, "life": 43, "ehp_pct": 26.7, "dps_pct": 2.8, "node": {"name": "Herbalism", "kind": "Notable"}}
                ]},
                "supports": {"extra": [], "missing": [{"gem": "Added Chaos Damage", "ehp_pct": 0, "dps_pct": 12.6}]},
                "gear": [{"reference_item": {"name": "Death Song, Short Bow", "base": "Short Bow"}, "slot": "Weapon 1", "dps_pct": 6.1, "ehp_pct": 0}],
                "reference": {"trees": [
                    {"active": false, "id": 1, "title": "Notes"},
                    {"active": true, "id": 2, "title": "Lvl 01-30"}
                ]}
            },
            "next_passives": [
                {"points": 8, "ehp_pct": 26.7, "dps_pct": 2.8, "node": {"name": "Herbalism"}},
                {"points": 2, "ehp_pct": 0, "dps_pct": 10, "node": {"name": "Heavy Draw"}}
            ],
            "later_stage": {"tree": {"active": false, "id": 3, "title": "Lvl 31-40"}}
        })
    }

    #[test]
    fn lists_gems_then_passives_then_gear() {
        let card = from_what_next(&answer());
        assert_eq!(card.stage.as_deref(), Some("Lvl 01-30"));
        assert_eq!(card.later_stage.as_deref(), Some("Lvl 31-40"));
        let names: Vec<&str> = card.steps.iter().map(|s| s.name.as_str()).collect();
        // Heavy Draw gives more per point than Herbalism.
        assert_eq!(
            names,
            [
                "Added Chaos Damage",
                "Heavy Draw",
                "Herbalism",
                "Death Song, Short Bow"
            ]
        );
        assert_eq!(card.steps[2].points, Some(8));
        assert_eq!(card.steps[3].slot.as_deref(), Some("Weapon 1"));
        assert_eq!(card.missing, None);
    }

    #[test]
    fn says_what_is_missing_in_the_players_words() {
        let no_guide = from_what_next(
            &json!({"status": {}, "guide": "No guide given, and the player follows none."}),
        );
        assert!(no_guide.missing.unwrap().starts_with("Follow a guide"));
        let no_build =
            from_what_next(&json!({"status": {}, "build": "The player has no build yet."}));
        assert!(no_build.missing.unwrap().contains("Import your build"));
        let done = from_what_next(&json!({"status": {}, "guide_diff": {}, "next_passives": []}));
        assert_eq!(
            done.missing.as_deref(),
            Some("Your build matches the guide's stage.")
        );
    }
}
