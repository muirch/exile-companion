//! Writes the first N route views as JSON, for the browser mock:
//! `cargo run -p poebo-core --example dump_route -- 14 > src/ipc/mock-route.json`

use std::collections::HashSet;

use poebo_core::data::DataPack;
use poebo_core::route::parse_route;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let count: usize = std::env::args().nth(1).map_or(Ok(14), |n| n.parse())?;
    let pack = DataPack::from_bytes(
        poebo_data::AREAS_JSON,
        poebo_data::AREAS_SCHEMA_VERSION,
        poebo_data::QUESTS_JSON,
        poebo_data::QUESTS_SCHEMA_VERSION,
        poebo_data::TRIALS_JSON,
        poebo_data::TRIALS_SCHEMA_VERSION,
    )?;
    let defines = HashSet::from(["LEAGUE_START".to_string()]);
    let source = poebo_data::ROUTE_ACTS.join("\n");
    let (route, _) = parse_route(&source, &pack.zones, &pack.quests, &defines);
    let views: Vec<_> = (0..count).filter_map(|e| route.view(e)).collect();
    println!("{}", serde_json::to_string_pretty(&views)?);
    Ok(())
}
