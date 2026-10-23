//! Runs the engine binary on a real Path of Building folder, named by EXILE_POB_DIR (a checkout or
//! an installed copy). Skipped without it, since PoB isn't part of this repo.

use std::path::{Path, PathBuf};
use std::time::Duration;

use exile_pob_engine::{Engine, EngineError};
use serde_json::{json, Value};

const FIXTURE: &str = include_str!("fixtures/ranger-caustic-arrow.xml");
// The same ranger as a guide would have it: Herbalism, a rare bow, Added Chaos Damage, Onslaught.
const GUIDE: &str = include_str!("fixtures/ranger-caustic-arrow-guide.xml");
const SLOW: Duration = Duration::from_secs(120);

fn pob_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("EXILE_POB_DIR").map(PathBuf::from);
    if dir.is_none() {
        eprintln!("EXILE_POB_DIR is not set, skipping");
    }
    dir
}

fn engine(pob: &Path) -> Engine {
    Engine::start(env!("CARGO_BIN_EXE_exile-pob-engine").as_ref(), pob, SLOW).unwrap()
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

#[test]
fn answers_questions_about_a_loaded_build() {
    let Some(pob) = pob_dir() else { return };
    let mut e = engine(&pob);

    let summary = e
        .call("load_build", json!({ "xml": FIXTURE }), SLOW)
        .unwrap();
    assert_eq!(summary["class"], "Ranger");
    assert_eq!(summary["level"], 10);
    assert_eq!(summary["main_skill"], "Caustic Arrow");
    assert!(f(&summary["offence"]["CombinedDPS"]) > 0.0);
    assert!(f(&summary["defence"]["Life"]) > 0.0);

    let defences = e.call("defences", json!({}), SLOW).unwrap();
    let types: Vec<&str> = defences["damage_types"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["type"].as_str().unwrap())
        .collect();
    assert_eq!(types, ["Physical", "Fire", "Cold", "Lightning", "Chaos"]);

    let gems = e.call("gem_value", json!({}), SLOW).unwrap();
    let support = &gems["gems"][1];
    assert_eq!(support["name"], "Void Manipulation");
    assert_eq!(support["support"], true);
    assert!(
        f(&support["dps_pct"]) < 0.0,
        "turning the support off should cost DPS: {support}"
    );

    // The fixture has only the class start, so the nearest notables are several points away.
    let power = e
        .call("node_power", json!({ "limit": 3, "max_points": 8 }), SLOW)
        .unwrap();
    assert_eq!(power["taken"], json!([]));
    let reachable = power["reachable"].as_array().unwrap();
    assert_eq!(reachable.len(), 3);
    assert!(reachable.iter().all(|r| r["points"].as_u64().unwrap() <= 8));

    let info = e.call("build_info", json!({}), SLOW).unwrap();
    assert_eq!(info["items"][0]["base"], "Short Bow");
    assert_eq!(info["socket_groups"][0]["main"], true);
}

#[test]
fn shows_the_sidebar_and_the_config_assumptions() {
    let Some(pob) = pob_dir() else { return };
    let mut e = engine(&pob);
    e.call("load_build", json!({ "xml": FIXTURE }), SLOW)
        .unwrap();

    let sidebar = e.call("sidebar_stats", json!({}), SLOW).unwrap();
    let life = sidebar["stats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["label"] == "Total Life")
        .expect("a Total Life row");
    assert!(
        life["value"].as_str().unwrap().parse::<f64>().is_ok(),
        "{life}"
    );

    let info = e.call("build_info", json!({}), SLOW).unwrap();
    assert!(
        info["config"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["var"] == "useFrenzyCharges"),
        "{}",
        info["config"]
    );
    // The commented-out line is off in PoB too.
    assert_eq!(
        info["custom_mods"],
        json!(["+10% to all Elemental Resistances"])
    );
}

#[test]
fn exports_a_code_that_loads_back() {
    let Some(pob) = pob_dir() else { return };
    let mut e = engine(&pob);
    let before = e
        .call("load_build", json!({ "xml": FIXTURE }), SLOW)
        .unwrap();
    let export = e.call("export_build", json!({}), SLOW).unwrap();
    let code = export["code"].as_str().unwrap();
    // zlib streams start with 0x78, which base64 writes as "e".
    assert!(code.starts_with('e'), "{code}");
    assert!(export["xml"].as_str().unwrap().contains("Caustic Arrow"));

    let after = e.call("load_code", json!({ "code": code }), SLOW).unwrap();
    assert_eq!(after["main_skill"], before["main_skill"]);
    assert_eq!(after["defence"]["Life"], before["defence"]["Life"]);

    let err = e
        .call("load_code", json!({ "code": "not a code" }), SLOW)
        .unwrap_err();
    assert!(
        err.to_string().contains("isn't a Path of Building code"),
        "{err}"
    );
}

const BOW: &str = "Item Class: Bows\nRarity: Rare\nDeath Song\nShort Bow\n--------\nItem Level: 30\n--------\nAdds 10 to 20 Physical Damage\n20% increased Attack Speed";

#[test]
fn compares_a_copied_item_with_the_equipped_one() {
    let Some(pob) = pob_dir() else { return };
    let mut e = engine(&pob);
    e.call("load_build", json!({ "xml": FIXTURE }), SLOW)
        .unwrap();

    let cmp = e
        .call("compare_item", json!({ "text": BOW }), SLOW)
        .unwrap();
    assert_eq!(cmp["item"]["base"], "Short Bow");
    let weapon = &cmp["slots"][0];
    assert_eq!(weapon["slot"], "Weapon 1");
    assert_eq!(weapon["replaces"], "Short Bow");
    assert!(f(&weapon["dps_pct"]) > 0.0, "{weapon}");

    let err = e
        .call("compare_item", json!({ "text": "hello" }), SLOW)
        .unwrap_err();
    assert!(err.to_string().contains("couldn't read that item"), "{err}");
}

#[test]
fn tries_changes_and_puts_the_build_back() {
    let Some(pob) = pob_dir() else { return };
    let mut e = engine(&pob);
    let before = e
        .call("load_build", json!({ "xml": FIXTURE }), SLOW)
        .unwrap();

    let more = e
        .call(
            "what_if",
            json!({ "changes": [
                { "add_gem": "Added Fire Damage Support", "group": "Caustic Arrow" },
                { "gem": "Caustic Arrow", "level": 10 },
                { "custom_mod": "+1 to Level of all Skill Gems" },
                { "level": 20 },
            ]}),
            SLOW,
        )
        .unwrap();
    assert!(f(&more["dps_pct"]) > 0.0, "{more}");
    assert!(more["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["stat"] == "Life"));

    let off = e
        .call(
            "what_if",
            json!({ "changes": [{ "gem": "Void Manipulation Support", "enabled": false }] }),
            SLOW,
        )
        .unwrap();
    assert!(f(&off["dps_pct"]) < 0.0, "{off}");

    let err = e
        .call("what_if", json!({ "changes": [{ "custom_mod": "+5 to Strength" }, { "add_gem": "No Such Gem" }] }), SLOW)
        .unwrap_err();
    assert!(err.to_string().contains("doesn't know the gem"), "{err}");

    let after = e.call("summary", json!({}), SLOW).unwrap();
    assert_eq!(after, before);
    let info = e.call("build_info", json!({}), SLOW).unwrap();
    assert_eq!(
        info["socket_groups"][0]["gems"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        info["custom_mods"],
        json!(["+10% to all Elemental Resistances"])
    );
}

#[test]
fn compares_the_build_with_a_guide() {
    let Some(pob) = pob_dir() else { return };
    let mut e = engine(&pob);
    let before = e
        .call("load_build", json!({ "xml": FIXTURE }), SLOW)
        .unwrap();
    let info = e
        .call(
            "load_reference",
            json!({ "xml": GUIDE, "label": "Test guide" }),
            SLOW,
        )
        .unwrap();
    assert_eq!(info["reference"], 1);
    assert_eq!(info["main_skill"], "Caustic Arrow");
    assert_eq!(info["trees"][0]["active"], true);

    let c = e
        .call("compare_reference", json!({ "reference": 1 }), SLOW)
        .unwrap();
    let missing = &c["passives"]["missing"][0];
    assert_eq!(missing["node"]["name"], "Herbalism");
    assert!(missing["points"].as_u64().unwrap() > 1, "{missing}");
    assert_eq!(c["passives"]["extra"], json!([]));
    assert_eq!(c["gear"][0]["slot"], "Weapon 1");
    assert_eq!(
        c["gear"][0]["reference_item"]["name"],
        "Death Song, Short Bow"
    );
    assert!(f(&c["gear"][0]["dps_pct"]) > 0.0);
    assert_eq!(c["supports"]["missing"][0]["gem"], "Added Chaos Damage");
    assert!(f(&c["supports"]["missing"][0]["dps_pct"]) > 0.0);
    assert_eq!(c["config"][0]["var"], "buffOnslaught");

    // With the guide, only what the guide takes counts as reachable, however far it is.
    let power = e
        .call("node_power", json!({ "reference": 1, "limit": 5 }), SLOW)
        .unwrap();
    let names: Vec<&str> = power["reachable"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["node"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Herbalism"]);

    // Comparing measured on the player's build and left it alone.
    assert_eq!(e.call("summary", json!({}), SLOW).unwrap(), before);
    let err = e
        .call("compare_reference", json!({ "reference": 9 }), SLOW)
        .unwrap_err();
    assert!(
        err.to_string().contains("load a reference build first"),
        "{err}"
    );
}

#[test]
fn reports_pob_errors_and_keeps_serving() {
    let Some(pob) = pob_dir() else { return };
    let mut e = engine(&pob);
    let err = e.call("no_such_method", json!({}), SLOW).unwrap_err();
    assert!(
        matches!(err, EngineError::Pob(ref m) if m.contains("no_such_method")),
        "{err}"
    );
    let err = e
        .call(
            "load_build",
            json!({ "xml": "<PathOfBuilding><Build" }),
            SLOW,
        )
        .unwrap_err();
    assert!(err.to_string().contains("couldn't open the build"), "{err}");
    e.call("load_build", json!({ "xml": FIXTURE }), SLOW)
        .unwrap();
}

#[test]
fn rejects_a_folder_that_is_not_pob() {
    let dir = tempfile::tempdir().unwrap();
    let exe = env!("CARGO_BIN_EXE_exile-pob-engine");
    let err = match Engine::start(exe.as_ref(), dir.path(), SLOW) {
        Ok(_) => panic!("started on an empty folder"),
        Err(e) => e,
    };
    assert!(
        matches!(err, EngineError::Exited(ref m) if m.contains("doesn't look like")),
        "{err}"
    );
}
