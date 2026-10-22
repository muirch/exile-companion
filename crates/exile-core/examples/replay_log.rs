//! Replays a Client.txt through exile-core and prints the campaign progression plus, at each step,
//! the guidance engine's next objective. Optionally import a Path of Building build to see gem/tree
//! guidance too.
//!
//! This is a headless way to see the engine work without the overlay (no GUI deps).
//!
//! Usage:
//!   cargo run -p exile-core --example replay_log                        # sample log, no build
//!   cargo run -p exile-core --example replay_log -- <log>               # a real Client.txt/recording
//!   cargo run -p exile-core --example replay_log -- <log> <build.xml>   # + a PoB build (file or code)

use std::sync::Arc;

use exile_core::build::BuildState;
use exile_core::data::DataPack;
use exile_core::guidance::GuidanceEngine;
use exile_core::locale::LocalePack;
use exile_core::parser::parse_line;
use exile_core::state::CampaignMachine;

fn load_pack() -> DataPack {
    DataPack::from_bytes(
        exile_data::AREAS_JSON,
        exile_data::AREAS_SCHEMA_VERSION,
        exile_data::QUESTS_JSON,
        exile_data::QUESTS_SCHEMA_VERSION,
        exile_data::TRIALS_JSON,
        exile_data::TRIALS_SCHEMA_VERSION,
    )
    .expect("failed to load the committed data pack")
}

fn main() {
    let mut args = std::env::args().skip(1);
    let log_path = args.next().unwrap_or_else(|| {
        format!(
            "{}/../../examples/sample-client.txt",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    let build_path = args.next();

    let text = std::fs::read_to_string(&log_path).expect("failed to read the log file");
    let data = load_pack();

    let mut build_state = BuildState::new();
    if let Some(path) = &build_path {
        match exile_pob::from_local_file(std::path::Path::new(path)) {
            Ok(build) => build_state.set(build),
            Err(e) => eprintln!("warning: could not import build {path}: {e}"),
        }
    }

    let mut machine = CampaignMachine::new(Arc::new(data.zones.clone()));

    println!("Replaying {log_path}");
    if let Some(summary) = build_state.summary() {
        println!(
            "Build: {} {} (main skill: {})",
            summary.class_name.as_deref().unwrap_or("?"),
            summary.ascend_class_name.as_deref().unwrap_or(""),
            summary.main_skill.as_deref().unwrap_or("?"),
        );
    }
    println!();
    println!(
        "{:>3} | {:>3} | {:<8} | {:<26} | next objective",
        "act", "lvl", "where", "zone"
    );
    println!("{}", "-".repeat(96));

    for line in text.lines() {
        let Some(event) = parse_line(line, &LocalePack::EN) else {
            continue;
        };
        let delta = machine.apply(event.event, event.at);
        if !delta.is_meaningful() {
            continue;
        }
        let s = machine.snapshot();
        let objective = GuidanceEngine::next(s, &build_state, &data);
        let act = s.act.map(|a| a.to_string()).unwrap_or_else(|| "-".into());
        let where_ = format!("{:?}", s.location_kind);
        let zone = s.zone.as_ref().map(|z| z.name.as_str()).unwrap_or("-");
        println!(
            "{act:>3} | {:>3} | {where_:<8} | {zone:<26} | {}",
            s.level, objective.headline
        );
    }

    let s = machine.snapshot();
    let objective = GuidanceEngine::next(s, &build_state, &data);
    println!("\nFinal state:");
    println!("  character : {:?} ({:?})", s.character, s.class);
    println!("  level     : {}", s.level);
    println!("  act       : {:?}", s.act);
    println!("  zone      : {:?}", s.zone.as_ref().map(|z| &z.name));
    println!("  deaths    : {}", s.deaths);
    println!(
        "  objective : {} - {}",
        objective.headline,
        objective.detail.as_deref().unwrap_or("")
    );
}
