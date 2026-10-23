//! Runs the engine binary on a real Path of Building folder, named by EXILE_POB_DIR (a checkout or
//! an installed copy). Skipped without it, since PoB isn't part of this repo.

use std::path::{Path, PathBuf};
use std::time::Duration;

use exile_pob_engine::{Engine, EngineError};
use serde_json::{json, Value};

const FIXTURE: &str = include_str!("fixtures/ranger-caustic-arrow.xml");
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
