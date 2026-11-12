//! The passive tree drawn from PoB's own tree data, and what a guide's stage marks on it: the
//! stage's tree, what it adds to the previous stage, or how it differs from the character's tree.

mod lua;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

use exile_pob::PobBuild;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::resolve::stage_for_level;

pub use lua::LuaError;

#[derive(Debug, thiserror::Error)]
pub enum TreeError {
    #[error("Path of Building has no {0} passive tree yet. Update Path of Building.")]
    Missing(String),
    #[error("Path of Building has no passive trees in {0}")]
    NoTrees(PathBuf),
    #[error("couldn't read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Lua(#[from] LuaError),
    #[error("tree.lua: {0}")]
    Shape(#[from] serde_json::Error),
}

/// PoB's tree file for `version` ("3_29"), or for its newest tree when the build names none.
pub fn tree_file(pob_src: &Path, version: Option<&str>) -> Result<PathBuf, TreeError> {
    let root = pob_src.join("TreeData");
    let version = match version {
        Some(v) => v.to_string(),
        None => newest_version(&root)?,
    };
    let file = root.join(&version).join("tree.lua");
    if file.is_file() {
        Ok(file)
    } else {
        Err(TreeError::Missing(version.replace('_', ".")))
    }
}

/// The newest tree Path of Building has, "3_30".
pub fn latest_version(pob_src: &Path) -> Result<String, TreeError> {
    newest_version(&pob_src.join("TreeData"))
}

fn newest_version(root: &Path) -> Result<String, TreeError> {
    let read = std::fs::read_dir(root).map_err(|source| TreeError::Read {
        path: root.to_path_buf(),
        source,
    })?;
    read.filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|name| {
            let (major, minor) = name.split_once('_')?;
            Some((
                (major.parse::<u32>().ok()?, minor.parse::<u32>().ok()?),
                name,
            ))
        })
        .max()
        .map(|(_, name)| name)
        .ok_or_else(|| TreeError::NoTrees(root.to_path_buf()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Normal,
    Notable,
    Keystone,
    Jewel,
    Mastery,
    AscendancyStart,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub kind: NodeKind,
    pub name: String,
    pub stats: Vec<String>,
    /// The ascendancy or bloodline the node belongs to. None on the main tree.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ascendancy: Option<String>,
    /// A mastery's effects to pick from.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mastery: Vec<MasteryEffect>,
    /// The game's explanation in brackets, "(Recently refers to the past 4 seconds)".
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reminder: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flavour: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasteryEffect {
    pub effect: u32,
    #[serde(default)]
    pub stats: Vec<String>,
}

/// A connection between two nodes. Nodes on the same orbit of a group are joined along the orbit.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeEdge {
    pub a: u32,
    pub b: u32,
    /// The orbit's radius when the edge is an arc. Drawn from `a` to `b` clockwise when `clockwise`.
    pub arc: Option<f64>,
    pub clockwise: bool,
}

/// The tree and the ascendancies without the class starts and the cluster jewel templates, in
/// PoB's tree coordinates (y grows down). Each ascendancy sits apart from the main tree.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeLayout {
    pub nodes: Vec<TreeNode>,
    pub edges: Vec<TreeEdge>,
    /// The name the game shows for each ascendancy and bloodline id, "Farrul" is "Farrul
    /// Bloodline".
    pub ascendancy_names: BTreeMap<String, String>,
    #[serde(skip)]
    index: HashMap<u32, usize>,
    /// Every node id the tree data has, also the ones not drawn (class starts, cluster parts).
    #[serde(skip)]
    known: HashSet<u32>,
}

#[derive(Deserialize)]
struct RawTree {
    groups: BTreeMap<String, RawGroup>,
    nodes: BTreeMap<String, RawNode>,
    constants: RawConstants,
    #[serde(default)]
    classes: Vec<RawClass>,
    #[serde(default)]
    alternate_ascendancies: Vec<RawAscendancy>,
}

#[derive(Deserialize)]
struct RawClass {
    #[serde(default)]
    ascendancies: Vec<RawAscendancy>,
}

#[derive(Deserialize)]
struct RawAscendancy {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct RawGroup {
    x: f64,
    y: f64,
    #[serde(default, rename = "isProxy")]
    is_proxy: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawConstants {
    skills_per_orbit: Vec<usize>,
    orbit_radii: Vec<f64>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawNode {
    name: Option<String>,
    stats: Vec<String>,
    group: Option<u32>,
    orbit: usize,
    orbit_index: usize,
    out: Vec<String>,
    #[serde(rename = "in")]
    into: Vec<String>,
    is_notable: bool,
    is_keystone: bool,
    is_jewel_socket: bool,
    is_mastery: bool,
    is_proxy: bool,
    is_ascendancy_start: bool,
    class_start_index: Option<u32>,
    ascendancy_name: Option<String>,
    expansion_jewel: Option<Value>,
    mastery_effects: Vec<MasteryEffect>,
    reminder_text: Vec<String>,
    flavour_text: Vec<String>,
}

/// Where a node sits on its orbit, as PoB places it. GGG's export spaces 16 and 40 node orbits at
/// every 30 and 45, and every 10 and 45 degrees.
fn orbit_angle(nodes_in_orbit: usize, index: usize) -> f64 {
    const SIXTEEN: [f64; 16] = [
        0., 30., 45., 60., 90., 120., 135., 150., 180., 210., 225., 240., 270., 300., 315., 330.,
    ];
    let degrees = match nodes_in_orbit {
        16 => SIXTEEN.get(index).copied().unwrap_or(0.),
        40 => {
            let tens = (0..36).map(|t| t as f64 * 10.);
            let mut all: Vec<f64> = tens.chain([45., 135., 225., 315.]).collect();
            all.sort_by(f64::total_cmp);
            all.get(index).copied().unwrap_or(0.)
        }
        0 => 0.,
        n => 360. * index as f64 / n as f64,
    };
    degrees.to_radians()
}

struct Placed {
    group: u32,
    orbit: usize,
    angle: f64,
}

fn round1(v: f64) -> f64 {
    (v * 10.).round() / 10.
}

impl TreeLayout {
    pub fn from_lua(src: &str) -> Result<Self, TreeError> {
        let raw: RawTree = serde_json::from_value(lua::parse(src)?)?;
        let RawConstants {
            skills_per_orbit,
            orbit_radii,
        } = raw.constants;
        let mut nodes = Vec::new();
        let mut placed = Vec::new();
        let mut links = Vec::new();
        for (key, n) in &raw.nodes {
            let Ok(id) = key.parse::<u32>() else {
                continue;
            };
            let in_cluster = n
                .expansion_jewel
                .as_ref()
                .is_some_and(|e| e.get("parent").is_some());
            if n.class_start_index.is_some() || n.is_proxy || in_cluster {
                continue;
            }
            let Some(group_id) = n.group else { continue };
            let Some(group) = raw.groups.get(&group_id.to_string()) else {
                continue;
            };
            if group.is_proxy {
                continue;
            }
            let per = skills_per_orbit.get(n.orbit).copied().unwrap_or(1);
            let radius = orbit_radii.get(n.orbit).copied().unwrap_or(0.);
            let angle = orbit_angle(per, n.orbit_index);
            let kind = if n.is_ascendancy_start {
                NodeKind::AscendancyStart
            } else if n.is_keystone {
                NodeKind::Keystone
            } else if n.is_notable {
                NodeKind::Notable
            } else if n.is_jewel_socket {
                NodeKind::Jewel
            } else if n.is_mastery {
                NodeKind::Mastery
            } else {
                NodeKind::Normal
            };
            nodes.push(TreeNode {
                id,
                x: round1(group.x + angle.sin() * radius),
                y: round1(group.y - angle.cos() * radius),
                kind,
                name: n.name.clone().unwrap_or_default(),
                stats: n.stats.clone(),
                ascendancy: n.ascendancy_name.clone().filter(|a| !a.is_empty()),
                mastery: n.mastery_effects.clone(),
                reminder: n.reminder_text.clone(),
                flavour: n.flavour_text.clone(),
            });
            placed.push(Placed {
                group: group_id,
                orbit: n.orbit,
                angle,
            });
            links.push(
                n.out
                    .iter()
                    .chain(&n.into)
                    .filter_map(|s| s.parse::<u32>().ok()),
            );
        }
        let index: HashMap<u32, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
        let mut seen = HashSet::new();
        let mut edges = Vec::new();
        for (i, targets) in links.into_iter().enumerate() {
            let a = &nodes[i];
            if a.kind == NodeKind::Mastery {
                continue;
            }
            for t in targets {
                let Some(&j) = index.get(&t) else { continue };
                let b = &nodes[j];
                if b.kind == NodeKind::Mastery
                    || a.ascendancy != b.ascendancy
                    || !seen.insert((a.id.min(t), a.id.max(t)))
                {
                    continue;
                }
                let (pa, pb) = (&placed[i], &placed[j]);
                let same_orbit = pa.group == pb.group && pa.orbit == pb.orbit && pa.orbit > 0;
                let arc = same_orbit
                    .then(|| orbit_radii.get(pa.orbit).copied())
                    .flatten();
                let turn = (pb.angle - pa.angle).rem_euclid(2. * PI);
                edges.push(TreeEdge {
                    a: a.id,
                    b: t,
                    arc,
                    clockwise: turn < PI,
                });
            }
        }
        // GGG's export keeps some ascendancy nodes that no longer link to anything, such as the
        // Guardian's Searing Purity, far from the rest.
        let linked: HashSet<u32> = edges.iter().flat_map(|e| [e.a, e.b]).collect();
        nodes.retain(|n| n.ascendancy.is_none() || linked.contains(&n.id));
        let index = nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
        let ascendancy_names = raw
            .classes
            .iter()
            .flat_map(|c| &c.ascendancies)
            .chain(&raw.alternate_ascendancies)
            .map(|a| (a.id.clone(), a.name.clone()))
            .collect();
        let known = raw.nodes.keys().filter_map(|k| k.parse().ok()).collect();
        Ok(Self {
            nodes,
            edges,
            ascendancy_names,
            index,
            known,
        })
    }

    pub fn node(&self, id: u32) -> Option<&TreeNode> {
        self.index.get(&id).map(|&i| &self.nodes[i])
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TreeMode {
    /// The stage's tree as PoB shows it.
    #[default]
    Tree,
    /// What the stage adds to the previous one, and what it refunds.
    New,
    /// What the character still has to take, and what it took that the stage doesn't.
    Character,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Mark {
    Taken,
    /// New at this stage, or still to take for the character.
    Added,
    /// Refunded at this stage, or taken by the character but not by the stage.
    Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeMark {
    pub id: u32,
    pub mark: Mark,
    /// The first stage of the guide that takes the node.
    pub since: Option<usize>,
    /// The effect picked on a mastery: the stage's, or for a node only the other tree has, that
    /// tree's.
    pub effect: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeMark {
    /// Index into [`TreeLayout::edges`].
    pub edge: usize,
    pub mark: Mark,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AscendancyMark {
    pub name: String,
    pub mark: Mark,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeStage {
    /// Every stage's title, in order.
    pub stages: Vec<String>,
    pub stage: usize,
    /// The tree data version the stage is for, "3_29".
    pub version: Option<String>,
    /// The mode shown. Character falls back to Tree without a character to compare with.
    pub mode: TreeMode,
    pub can_compare: bool,
    pub points: usize,
    pub notables: usize,
    pub added: usize,
    pub extra: usize,
    /// Notables and keystones marked Added, in tree order.
    pub added_names: Vec<String>,
    /// The ascendancy notables, in the order the guide takes them.
    pub ascendancy: Vec<AscendancyMark>,
    /// The ascendancies with marked nodes, the guide's own first, then a bloodline.
    pub ascendancies: Vec<String>,
    pub nodes: Vec<NodeMark>,
    pub edges: Vec<EdgeMark>,
    /// What the marked nodes cover. None when nothing is marked.
    pub focus: Option<Bounds>,
    /// The newest tree Path of Building has when the stage is for an older one. Set by the app.
    pub latest: Option<String>,
    /// The compared character's tree version when it isn't the stage's, "3_30".
    pub character_version: Option<String>,
    /// Nodes of the stage or the compared tree this tree data doesn't have, named from the
    /// character's tree when it has them. Cluster jewel nodes don't count.
    pub missing: usize,
    pub missing_names: Vec<String>,
    #[serde(skip)]
    pub missing_ids: Vec<u32>,
}

pub fn stage_titles(build: &PobBuild) -> Vec<String> {
    build
        .specs
        .iter()
        .enumerate()
        .map(|(i, s)| s.title.clone().unwrap_or_else(|| format!("Tree {}", i + 1)))
        .collect()
}

/// The tree a character of `level` is at: the one whose title names the highest level not above
/// it, else the one the build has active.
pub fn stage_for(build: &PobBuild, level: u32) -> usize {
    let titles = stage_titles(build);
    stage_for_level(titles.iter().map(String::as_str), level).unwrap_or_else(|| {
        build
            .active_spec
            .saturating_sub(1)
            .min(build.specs.len().saturating_sub(1))
    })
}

fn edge_mark(a: Mark, b: Mark) -> Option<Mark> {
    use Mark::*;
    match (a, b) {
        (Taken, Taken) => Some(Taken),
        (Added, Added) | (Taken, Added) | (Added, Taken) => Some(Added),
        (Extra, Extra) | (Taken, Extra) | (Extra, Taken) => Some(Extra),
        (Added, Extra) | (Extra, Added) => None,
    }
}

/// The marks for stage `stage` of `guide` in `mode`. `character` is the tree to compare with.
pub fn tree_stage(
    layout: &TreeLayout,
    guide: &PobBuild,
    stage: usize,
    character: Option<&PobBuild>,
    mode: TreeMode,
) -> TreeStage {
    let stage = stage.min(guide.specs.len().saturating_sub(1));
    let spec = guide.specs.get(stage);
    let now: HashSet<u32> = spec
        .map(|s| s.nodes.iter().copied().collect())
        .unwrap_or_default();
    let my_spec = character.and_then(PobBuild::selected_spec);
    let mine: Option<HashSet<u32>> = my_spec.map(|s| s.nodes.iter().copied().collect());
    let mode = match (mode, &mine) {
        (TreeMode::Character, None) => TreeMode::Tree,
        (m, _) => m,
    };
    let before_spec = match mode {
        TreeMode::Tree => spec,
        TreeMode::New => stage.checked_sub(1).and_then(|i| guide.specs.get(i)),
        TreeMode::Character => my_spec,
    };
    let before: HashSet<u32> = before_spec
        .map(|s| s.nodes.iter().copied().collect())
        .unwrap_or_default();
    let effect = |id: u32| {
        let pick = |s: Option<&exile_pob::TreeSpec>| {
            s?.mastery_effects
                .iter()
                .find(|(node, _)| *node == id)
                .map(|(_, e)| *e)
        };
        pick(spec).or_else(|| pick(before_spec))
    };
    let mark_of = |id: u32| match (now.contains(&id), before.contains(&id)) {
        (true, true) => Some(Mark::Taken),
        (true, false) => Some(Mark::Added),
        (false, true) => Some(Mark::Extra),
        (false, false) => None,
    };
    let since = |id: u32| guide.specs.iter().position(|s| s.nodes.contains(&id));

    let mut marks: HashMap<u32, Mark> = HashMap::new();
    let mut nodes = Vec::new();
    let mut added_names = Vec::new();
    let (mut points, mut notables, mut added, mut extra) = (0, 0, 0, 0);
    let mut focus: Option<Bounds> = None;
    let mut ascendancy = Vec::new();
    let mut ascendancies: Vec<String> = Vec::new();
    for n in &layout.nodes {
        let Some(mark) = mark_of(n.id) else { continue };
        marks.insert(n.id, mark);
        nodes.push(NodeMark {
            id: n.id,
            mark,
            since: since(n.id),
            effect: (n.kind == NodeKind::Mastery)
                .then(|| effect(n.id))
                .flatten(),
        });
        if let Some(asc) = &n.ascendancy {
            if !ascendancies.contains(asc) {
                ascendancies.push(asc.clone());
            }
            if n.kind == NodeKind::Notable {
                ascendancy.push((
                    since(n.id),
                    AscendancyMark {
                        name: n.name.clone(),
                        mark,
                    },
                ));
            }
            continue;
        }
        if mark != Mark::Extra {
            points += 1;
            notables += usize::from(n.kind == NodeKind::Notable);
        }
        match mark {
            Mark::Added => {
                added += 1;
                if matches!(n.kind, NodeKind::Notable | NodeKind::Keystone) {
                    added_names.push(n.name.clone());
                }
            }
            Mark::Extra => extra += 1,
            Mark::Taken => {}
        }
        let b = focus.get_or_insert(Bounds {
            min_x: n.x,
            min_y: n.y,
            max_x: n.x,
            max_y: n.y,
        });
        b.min_x = b.min_x.min(n.x);
        b.min_y = b.min_y.min(n.y);
        b.max_x = b.max_x.max(n.x);
        b.max_y = b.max_y.max(n.y);
    }
    let edges = layout
        .edges
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let mark = edge_mark(*marks.get(&e.a)?, *marks.get(&e.b)?)?;
            Some(EdgeMark { edge: i, mark })
        })
        .collect();
    ascendancy.sort_by_key(|(since, _)| since.unwrap_or(usize::MAX));
    ascendancies.sort_by_key(|a| Some(a) != guide.ascend_class_name.as_ref());
    let version = spec.and_then(|s| s.tree_version.clone());
    let character_version = my_spec
        .and_then(|s| s.tree_version.clone())
        .filter(|v| Some(v) != version.as_ref());
    let mut missing_ids: Vec<u32> = now
        .union(&before)
        .copied()
        .filter(|id| *id < CLUSTER_NODES && !layout.known.contains(id))
        .collect();
    missing_ids.sort_unstable();
    TreeStage {
        stages: stage_titles(guide),
        stage,
        version,
        mode,
        can_compare: mine.is_some(),
        points,
        notables,
        added,
        extra,
        added_names,
        ascendancy: ascendancy.into_iter().map(|(_, a)| a).collect(),
        ascendancies,
        nodes,
        edges,
        focus,
        latest: None,
        character_version,
        missing: missing_ids.len(),
        missing_names: Vec::new(),
        missing_ids,
    }
}

/// PoB numbers the nodes it builds for cluster jewels from here up.
const CLUSTER_NODES: u32 = 0x10000;

impl TreeStage {
    /// Names the missing nodes from another version's tree, the character's.
    pub fn name_missing(&mut self, other: &TreeLayout) {
        self.missing_names = self
            .missing_ids
            .iter()
            .filter_map(|id| other.node(*id))
            .filter(|n| !matches!(n.kind, NodeKind::Normal | NodeKind::Mastery))
            .map(|n| n.name.clone())
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use exile_pob::TreeSpec;

    use super::*;

    // Group 1 at the origin with a 16-node orbit (2) and a 6-node orbit (1), group 2 to the right.
    // 10-11 sit on one orbit (an arc), 11-20 cross groups (a line), 12 is a mastery, 30 is an
    // ascendancy notable in group 4, 31 its start, 32 an ascendancy node linked to nothing, 40 a
    // class start that links to 31, 50 sits in a cluster jewel proxy group.
    const TREE: &str = r#"return {
    ["groups"]= {
        [1]= { ["x"]= 0, ["y"]= 0, ["orbits"]= { 0, 1, 2 } },
        [2]= { ["x"]= 1000, ["y"]= 0 },
        [3]= { ["x"]= 5000, ["y"]= 5000, ["isProxy"]= true },
        [4]= { ["x"]= -8000, ["y"]= 9000 },
        [5]= { ["x"]= -1000, ["y"]= 13000 },
    },
    ["nodes"]= {
        ["root"]= { ["group"]= 0, ["out"]= { "40" }, ["in"]= {} },
        [10]= { ["skill"]= 10, ["name"]= "Strength", ["stats"]= { "+10 to Strength" }, ["group"]= 1, ["orbit"]= 2, ["orbitIndex"]= 0, ["out"]= { "11" }, ["in"]= {} },
        [11]= { ["skill"]= 11, ["name"]= "Heart", ["isNotable"]= true, ["stats"]= {}, ["group"]= 1, ["orbit"]= 2, ["orbitIndex"]= 4, ["out"]= { "20" }, ["in"]= { "10" } },
        [12]= { ["skill"]= 12, ["name"]= "Life Mastery", ["isMastery"]= true, ["group"]= 1, ["orbit"]= 0, ["orbitIndex"]= 0, ["out"]= {}, ["in"]= { "10" },
            ["masteryEffects"]= { { ["effect"]= 9755, ["stats"]= { "+50 to maximum Life" } }, { ["effect"]= 4119, ["stats"]= { "Regenerate 1% of Life per second" } } } },
        [13]= { ["skill"]= 13, ["name"]= "Socket", ["isJewelSocket"]= true, ["group"]= 1, ["orbit"]= 1, ["orbitIndex"]= 3, ["out"]= {}, ["in"]= { "10" } },
        [20]= { ["skill"]= 20, ["name"]= "Iron Reflexes", ["isKeystone"]= true, ["reminderText"]= { "(Armour reduces Physical Damage)" }, ["flavourText"]= { "Steel yourself." }, ["group"]= 2, ["orbit"]= 0, ["orbitIndex"]= 0, ["out"]= {}, ["in"]= { "11" } },
        [30]= { ["skill"]= 30, ["name"]= "Radiant Crusade", ["isNotable"]= true, ["ascendancyName"]= "Guardian", ["group"]= 4, ["orbit"]= 1, ["orbitIndex"]= 0, ["out"]= {}, ["in"]= { "31" } },
        [31]= { ["skill"]= 31, ["name"]= "Guardian", ["isAscendancyStart"]= true, ["ascendancyName"]= "Guardian", ["group"]= 4, ["orbit"]= 0, ["orbitIndex"]= 0, ["out"]= { "30" }, ["in"]= { "40" } },
        [32]= { ["skill"]= 32, ["name"]= "Searing Purity", ["isNotable"]= true, ["ascendancyName"]= "Guardian", ["group"]= 2, ["orbit"]= 0, ["orbitIndex"]= 0, ["out"]= {}, ["in"]= {} },
        [60]= { ["skill"]= 60, ["name"]= "Farrul Bloodline", ["isAscendancyStart"]= true, ["isBloodline"]= true, ["ascendancyName"]= "Farrul", ["group"]= 5, ["orbit"]= 0, ["orbitIndex"]= 0, ["out"]= { "61" }, ["in"]= {} },
        [61]= { ["skill"]= 61, ["name"]= "Huntleader", ["isNotable"]= true, ["isBloodline"]= true, ["ascendancyName"]= "Farrul", ["group"]= 5, ["orbit"]= 1, ["orbitIndex"]= 0, ["out"]= {}, ["in"]= { "60" } },
        [40]= { ["skill"]= 40, ["name"]= "TEMPLAR", ["classStartIndex"]= 5, ["group"]= 2, ["orbit"]= 0, ["orbitIndex"]= 0, ["out"]= { "20", "31" }, ["in"]= {} },
        [50]= { ["skill"]= 50, ["name"]= "Cluster", ["group"]= 3, ["orbit"]= 0, ["orbitIndex"]= 0, ["out"]= {}, ["in"]= {} },
    },
    ["classes"]= {
        { ["name"]= "Templar", ["ascendancies"]= { { ["id"]= "Inquisitor", ["name"]= "Inquisitor" }, { ["id"]= "Guardian", ["name"]= "Guardian" } } },
    },
    ["alternate_ascendancies"]= {
        { ["id"]= "Farrul", ["name"]= "Farrul Bloodline", ["flavourText"]= "You and your pack run wild" },
    },
    ["constants"]= {
        ["skillsPerOrbit"]= { 1, 6, 16, 16, 40, 72, 72 },
        ["orbitRadii"]= { 0, 82, 162, 335, 493, 662, 846 },
    },
}"#;

    fn layout() -> TreeLayout {
        TreeLayout::from_lua(TREE).unwrap()
    }

    fn spec(title: &str, nodes: &[u32]) -> TreeSpec {
        TreeSpec {
            title: Some(title.into()),
            tree_version: Some("3_29".into()),
            nodes: nodes.to_vec(),
            ..TreeSpec::default()
        }
    }

    fn guide() -> PobBuild {
        PobBuild {
            specs: vec![
                spec("Level 1-12", &[10, 13]),
                spec("Level 12-22", &[10, 11, 20, 30]),
            ],
            ..PobBuild::default()
        }
    }

    fn character(nodes: &[u32]) -> PobBuild {
        PobBuild {
            specs: vec![spec("Character", nodes)],
            ..PobBuild::default()
        }
    }

    #[test]
    fn places_nodes_where_pob_does() {
        let l = layout();
        let ids: Vec<u32> = l.nodes.iter().map(|n| n.id).collect();
        assert_eq!(ids, vec![10, 11, 12, 13, 20, 30, 31, 60, 61]);
        assert_eq!(
            l.ascendancy_names.get("Farrul").map(String::as_str),
            Some("Farrul Bloodline")
        );
        assert_eq!(
            l.ascendancy_names.get("Guardian").map(String::as_str),
            Some("Guardian")
        );
        let at = |id| {
            let n = l.node(id).unwrap();
            (n.x, n.y)
        };
        assert_eq!(at(10), (0., -162.));
        // Index 4 on a 16-node orbit is 90 degrees, not 4 * 22.5.
        assert_eq!(at(11), (162., 0.));
        // A 6-node orbit is even: index 3 is straight down.
        assert_eq!(at(13), (0., 82.));
        assert_eq!(at(20), (1000., 0.));
        assert_eq!(l.node(20).unwrap().kind, NodeKind::Keystone);
        assert_eq!(
            l.node(20).unwrap().reminder,
            vec!["(Armour reduces Physical Damage)"]
        );
        assert_eq!(l.node(20).unwrap().flavour, vec!["Steel yourself."]);
        assert!(l.node(10).unwrap().reminder.is_empty());
        assert_eq!(l.node(13).unwrap().kind, NodeKind::Jewel);
        assert_eq!(at(30), (-8000., 8918.));
        assert_eq!(l.node(30).unwrap().ascendancy.as_deref(), Some("Guardian"));
        assert_eq!(l.node(31).unwrap().kind, NodeKind::AscendancyStart);
        assert_eq!(l.node(10).unwrap().ascendancy, None);
        let effects: Vec<u32> = l
            .node(12)
            .unwrap()
            .mastery
            .iter()
            .map(|m| m.effect)
            .collect();
        assert_eq!(effects, vec![9755, 4119]);
    }

    #[test]
    fn forty_node_orbits_add_the_diagonals() {
        assert_eq!(orbit_angle(40, 5), 45f64.to_radians());
        assert_eq!(orbit_angle(40, 6), 50f64.to_radians());
        assert_eq!(orbit_angle(40, 39), 350f64.to_radians());
        assert_eq!(orbit_angle(72, 18), 90f64.to_radians());
    }

    #[test]
    fn joins_an_orbit_with_an_arc_and_skips_masteries() {
        let l = layout();
        assert_eq!(
            l.edges[..4],
            vec![
                TreeEdge {
                    a: 10,
                    b: 11,
                    arc: Some(162.),
                    clockwise: true
                },
                TreeEdge {
                    a: 11,
                    b: 20,
                    arc: None,
                    clockwise: false
                },
                TreeEdge {
                    a: 13,
                    b: 10,
                    arc: None,
                    clockwise: false
                },
                TreeEdge {
                    a: 30,
                    b: 31,
                    arc: None,
                    clockwise: true
                },
            ]
        );
    }

    #[test]
    fn tree_mode_marks_the_stage_as_pob_shows_it() {
        let s = tree_stage(&layout(), &guide(), 1, None, TreeMode::Tree);
        assert_eq!(s.mode, TreeMode::Tree);
        assert!(!s.can_compare);
        assert!(s.nodes.iter().all(|n| n.mark == Mark::Taken));
        assert_eq!(
            s.nodes.iter().map(|n| n.id).collect::<Vec<_>>(),
            vec![10, 11, 20, 30]
        );
        assert_eq!(s.ascendancies, vec!["Guardian"]);
        assert_eq!((s.points, s.notables, s.added, s.extra), (3, 1, 0, 0));
        assert_eq!(s.edges.len(), 2);
        assert_eq!(
            s.ascendancy,
            vec![AscendancyMark {
                name: "Radiant Crusade".into(),
                mark: Mark::Taken
            }]
        );
        assert_eq!(s.version.as_deref(), Some("3_29"));
        assert_eq!(
            s.focus,
            Some(Bounds {
                min_x: 0.,
                min_y: -162.,
                max_x: 1000.,
                max_y: 0.
            })
        );
    }

    #[test]
    fn new_mode_marks_what_the_stage_adds_and_refunds() {
        let s = tree_stage(&layout(), &guide(), 1, None, TreeMode::New);
        let marks: Vec<(u32, Mark)> = s.nodes.iter().map(|n| (n.id, n.mark)).collect();
        assert_eq!(
            marks,
            vec![
                (10, Mark::Taken),
                (11, Mark::Added),
                (13, Mark::Extra),
                (20, Mark::Added),
                (30, Mark::Added)
            ]
        );
        assert_eq!(s.added_names, vec!["Heart", "Iron Reflexes"]);
        assert_eq!((s.points, s.added, s.extra), (3, 2, 1));
        assert_eq!(s.ascendancy[0].mark, Mark::Added);
        let edge = |i: usize| s.edges.iter().find(|e| e.edge == i).map(|e| e.mark);
        assert_eq!(edge(0), Some(Mark::Added));
        assert_eq!(edge(1), Some(Mark::Added));
        assert_eq!(edge(2), Some(Mark::Extra));
        assert_eq!(s.nodes[1].since, Some(1));
        assert_eq!(s.nodes[0].since, Some(0));
    }

    #[test]
    fn a_mastery_carries_the_picked_effect() {
        let mut g = guide();
        g.specs[1].nodes.push(12);
        g.specs[1].mastery_effects = vec![(12, 4119)];
        let s = tree_stage(&layout(), &g, 1, None, TreeMode::Tree);
        let mastery = s.nodes.iter().find(|n| n.id == 12).unwrap();
        assert_eq!(mastery.effect, Some(4119));
        assert!(s
            .nodes
            .iter()
            .filter(|n| n.id != 12)
            .all(|n| n.effect.is_none()));
        // Refunded at the next stage, it keeps the effect the stage before had.
        let next = PobBuild {
            specs: vec![g.specs[1].clone(), spec("Later", &[10])],
            ..PobBuild::default()
        };
        let s = tree_stage(&layout(), &next, 1, None, TreeMode::New);
        let mastery = s.nodes.iter().find(|n| n.id == 12).unwrap();
        assert_eq!((mastery.mark, mastery.effect), (Mark::Extra, Some(4119)));
    }

    #[test]
    fn draws_a_bloodline_after_the_guides_ascendancy() {
        let mut g = PobBuild {
            ascend_class_name: Some("Guardian".into()),
            ..guide()
        };
        g.specs[1].nodes.extend([60, 61]);
        let s = tree_stage(&layout(), &g, 1, None, TreeMode::Tree);
        assert_eq!(s.ascendancies, vec!["Guardian", "Farrul"]);
        let names: Vec<&str> = s.ascendancy.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["Radiant Crusade", "Huntleader"]);
        // Bloodline points don't count as tree points.
        assert_eq!(s.points, 3);
        // Before the stage takes a point there, an ascendancy isn't drawn at all.
        let s = tree_stage(&layout(), &g, 0, None, TreeMode::Tree);
        assert!(s.ascendancies.is_empty());
    }

    #[test]
    fn the_first_stage_is_all_new() {
        let s = tree_stage(&layout(), &guide(), 0, None, TreeMode::New);
        assert!(s.nodes.iter().all(|n| n.mark == Mark::Added));
    }

    #[test]
    fn character_mode_compares_with_the_character() {
        let me = character(&[10, 13]);
        let s = tree_stage(&layout(), &guide(), 1, Some(&me), TreeMode::Character);
        assert_eq!(s.mode, TreeMode::Character);
        assert!(s.can_compare);
        let marks: Vec<(u32, Mark)> = s.nodes.iter().map(|n| (n.id, n.mark)).collect();
        assert_eq!(
            marks,
            vec![
                (10, Mark::Taken),
                (11, Mark::Added),
                (13, Mark::Extra),
                (20, Mark::Added),
                (30, Mark::Added)
            ]
        );
    }

    #[test]
    fn a_character_on_a_newer_tree_says_what_this_tree_lacks() {
        let mut me = character(&[10, 11, 40, 99, 0x10003]);
        me.specs[0].tree_version = Some("3_30".into());
        let mut s = tree_stage(&layout(), &guide(), 1, Some(&me), TreeMode::Character);
        assert_eq!(s.character_version.as_deref(), Some("3_30"));
        // 40 is the class start and 0x10003 a cluster jewel node, both fine.
        assert_eq!(s.missing, 1);
        let newer = TreeLayout::from_lua(&TREE.replace("[50]=", "[99]= { [\"skill\"]= 99, [\"name\"]= \"Bastion\", [\"isNotable\"]= true, [\"group\"]= 2, [\"orbit\"]= 0, [\"orbitIndex\"]= 0, [\"out\"]= {}, [\"in\"]= {} },\n        [50]=")).unwrap();
        s.name_missing(&newer);
        assert_eq!(s.missing_names, vec!["Bastion"]);

        let same = character(&[10, 11]);
        let s = tree_stage(&layout(), &guide(), 1, Some(&same), TreeMode::Character);
        assert_eq!(s.character_version, None);
        assert_eq!(s.missing, 0);
    }

    #[test]
    fn character_mode_without_a_character_shows_the_tree() {
        let s = tree_stage(&layout(), &guide(), 1, None, TreeMode::Character);
        assert_eq!(s.mode, TreeMode::Tree);
        assert!(!s.can_compare);
    }

    #[test]
    fn stage_out_of_range_is_the_last_one() {
        assert_eq!(
            tree_stage(&layout(), &guide(), 9, None, TreeMode::Tree).stage,
            1
        );
        let empty = tree_stage(&layout(), &PobBuild::default(), 0, None, TreeMode::Tree);
        assert!(empty.nodes.is_empty() && empty.focus.is_none());
    }

    #[test]
    fn picks_the_stage_for_the_level() {
        assert_eq!(stage_for(&guide(), 5), 0);
        assert_eq!(stage_for(&guide(), 30), 1);
        let untitled = PobBuild {
            specs: vec![TreeSpec::default(), TreeSpec::default()],
            active_spec: 2,
            ..PobBuild::default()
        };
        assert_eq!(stage_for(&untitled, 30), 1);
        assert_eq!(stage_titles(&untitled), vec!["Tree 1", "Tree 2"]);
    }

    #[test]
    fn finds_the_tree_file_for_a_version() {
        let dir = tempfile::tempdir().unwrap();
        for v in ["3_9", "3_29", "3_29_ruthless", "3_28"] {
            std::fs::create_dir_all(dir.path().join("TreeData").join(v)).unwrap();
            std::fs::write(dir.path().join("TreeData").join(v).join("tree.lua"), "{}").unwrap();
        }
        let file = tree_file(dir.path(), None).unwrap();
        assert!(file.ends_with("TreeData/3_29/tree.lua"));
        assert!(tree_file(dir.path(), Some("3_28"))
            .unwrap()
            .ends_with("3_28/tree.lua"));
        let err = tree_file(dir.path(), Some("3_30")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Path of Building has no 3.30 passive tree yet. Update Path of Building."
        );
    }

    /// Reads the PoB copy the engine tests use. `cargo test -- --ignored` with `.spike/pob` in place.
    #[test]
    #[ignore]
    fn reads_pobs_own_tree() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.spike/pob/src");
        let file = tree_file(&root, Some("3_29")).unwrap();
        let l = TreeLayout::from_lua(&std::fs::read_to_string(file).unwrap()).unwrap();
        assert!(l.nodes.len() > 2300, "{} nodes", l.nodes.len());
        assert!(l.edges.len() > 2300, "{} edges", l.edges.len());
        let crusade = l
            .nodes
            .iter()
            .find(|n| n.name == "Radiant Crusade")
            .unwrap();
        assert_eq!(crusade.ascendancy.as_deref(), Some("Guardian"));
        assert!(!l.nodes.iter().any(|n| n.name == "Searing Purity"));
        assert!(l
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Mastery)
            .all(|n| !n.mastery.is_empty()));
    }
}
