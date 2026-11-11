//! Writes the browser mock's passive tree: part of PoB's tree around a guide's first stages with
//! the guide's ascendancy, and each of those stages in every mode, against a made-up character.
//!
//! cargo run -p exile-core --example dump_tree -- <PoB src> <guide.xml> > app/ui/src/ipc/mock-tree.json

use std::collections::HashSet;
use std::path::Path;

use exile_core::tree::{tree_file, tree_stage, TreeLayout, TreeMode};
use exile_pob::{PobBuild, TreeSpec};

const STAGES: usize = 5;
const MARGIN: f64 = 2500.;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [pob, guide] = args.as_slice() else {
        return Err("usage: dump_tree <PoB src> <guide.xml>".into());
    };
    let guide = exile_pob::from_local_file(Path::new(guide))?;
    let version = guide.specs.first().and_then(|s| s.tree_version.clone());
    let mut layout = TreeLayout::from_lua(&std::fs::read_to_string(tree_file(
        Path::new(pob),
        version.as_deref(),
    )?)?)?;

    // The second stage plus the first three neighbours the third stage doesn't take.
    let mut mine: Vec<u32> = guide
        .specs
        .get(1)
        .map(|s| s.nodes.clone())
        .unwrap_or_default();
    let next: HashSet<u32> = guide
        .specs
        .get(2)
        .map(|s| s.nodes.iter().copied().collect())
        .unwrap_or_default();
    let taken: HashSet<u32> = mine.iter().copied().collect();
    let extra: Vec<u32> = layout
        .edges
        .iter()
        .filter_map(|e| match (taken.contains(&e.a), taken.contains(&e.b)) {
            (true, false) => Some(e.b),
            (false, true) => Some(e.a),
            _ => None,
        })
        .filter(|id| {
            !next.contains(id)
                && layout
                    .node(*id)
                    .is_some_and(|n| n.stats.len() == 1 && n.ascendancy.is_none())
        })
        .take(3)
        .collect();
    mine.extend(extra);
    let character = PobBuild {
        specs: vec![TreeSpec {
            nodes: mine,
            ..TreeSpec::default()
        }],
        ..PobBuild::default()
    };

    let mut stages = Vec::new();
    for stage in 0..STAGES.min(guide.specs.len()) {
        for mode in [TreeMode::Tree, TreeMode::New, TreeMode::Character] {
            stages.push(tree_stage(&layout, &guide, stage, Some(&character), mode));
        }
    }
    let focus = stages
        .iter()
        .filter_map(|s| s.focus)
        .reduce(|a, b| exile_core::tree::Bounds {
            min_x: a.min_x.min(b.min_x),
            min_y: a.min_y.min(b.min_y),
            max_x: a.max_x.max(b.max_x),
            max_y: a.max_y.max(b.max_y),
        })
        .ok_or("the guide's first stages mark nothing")?;
    let drawn: HashSet<&String> = stages.iter().flat_map(|s| &s.ascendancies).collect();
    let inside = |id: u32| {
        layout.node(id).is_some_and(|n| {
            n.ascendancy.as_ref().is_some_and(|a| drawn.contains(a))
                || (n.ascendancy.is_none()
                    && n.x > focus.min_x - MARGIN
                    && n.x < focus.max_x + MARGIN
                    && n.y > focus.min_y - MARGIN
                    && n.y < focus.max_y + MARGIN)
        })
    };
    let keep: HashSet<u32> = layout
        .nodes
        .iter()
        .map(|n| n.id)
        .filter(|id| inside(*id))
        .collect();
    // Edge indices in the stages point into the full list, so cut edges to empty ones, not out.
    for e in &mut layout.edges {
        if !(keep.contains(&e.a) && keep.contains(&e.b)) {
            e.a = 0;
            e.b = 0;
        }
    }
    layout.nodes.retain(|n| keep.contains(&n.id));
    let out = serde_json::json!({ "layout": layout, "stages": stages });
    println!("{}", serde_json::to_string(&out)?);
    Ok(())
}
