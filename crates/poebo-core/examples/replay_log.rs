//! Replays a Client.txt through poebo-core and prints the campaign progression.
//!
//! This is a headless way to see the engine work without the overlay (no GUI deps).
//!
//! Usage:
//!   cargo run -p poebo-core --example replay_log            # uses examples/sample-client.txt
//!   cargo run -p poebo-core --example replay_log -- <path>  # a real Client.txt or recording

use std::sync::Arc;

use poebo_core::data::ZoneTable;
use poebo_core::locale::LocalePack;
use poebo_core::parser::parse_line;
use poebo_core::state::CampaignMachine;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        format!(
            "{}/../../examples/sample-client.txt",
            env!("CARGO_MANIFEST_DIR")
        )
    });

    let text = std::fs::read_to_string(&path).expect("failed to read the log file");
    let zones = ZoneTable::from_json_bytes(poebo_data::AREAS_JSON, poebo_data::PACK_SCHEMA_VERSION)
        .expect("failed to load the committed data pack");
    let mut machine = CampaignMachine::new(Arc::new(zones));

    println!("Replaying {path}\n");
    println!("{:>3} | {:>3} | {:<8} | zone", "act", "lvl", "where");
    println!("{}", "-".repeat(48));

    for line in text.lines() {
        let Some(event) = parse_line(line, &LocalePack::EN) else {
            continue;
        };
        let delta = machine.apply(event.event, event.at);
        if !delta.is_meaningful() {
            continue;
        }
        let s = machine.snapshot();
        let act = s.act.map(|a| a.to_string()).unwrap_or_else(|| "-".into());
        let where_ = format!("{:?}", s.location_kind);
        let zone = s.zone.as_ref().map(|z| z.name.as_str()).unwrap_or("-");
        println!("{act:>3} | {:>3} | {where_:<8} | {zone}", s.level);
    }

    let s = machine.snapshot();
    println!("\nFinal state:");
    println!("  character : {:?} ({:?})", s.character, s.class);
    println!("  level     : {}", s.level);
    println!("  act       : {:?}", s.act);
    println!("  zone      : {:?}", s.zone.as_ref().map(|z| &z.name));
    println!("  deaths    : {}", s.deaths);
}
