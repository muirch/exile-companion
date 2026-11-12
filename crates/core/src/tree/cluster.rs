//! Cluster jewel passives, built the way PoB builds them (`PassiveSpec:BuildSubgraph`): the nodes a
//! cluster jewel adds don't exist in the tree data, only where they go (the socket's proxy group)
//! and how they are numbered. Node ids must match PoB's, since a build lists the taken ones by id.

use std::collections::HashMap;
use std::f64::consts::PI;

use exile_pob::{ItemText, PobBuild, TreeSpec};
use serde_json::Value;

use super::{lua, orbit_angle, round1, NodeKind, TreeEdge, TreeError, TreeNode};

/// PoB numbers cluster nodes from here up, clear of the tree's own node ids.
const FIRST_ID: u32 = 0x10000;

/// What the tree data says about cluster jewels: the sockets and where their passives go.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct ClusterTree {
    pub sockets: HashMap<u32, Socket>,
    /// The group of each proxy node, where a socket's passives are laid out.
    pub proxies: HashMap<u32, u32>,
    pub groups: HashMap<u32, (f64, f64)>,
    /// The sockets in each group, by their index in it.
    pub group_sockets: HashMap<u32, Vec<u32>>,
    /// The notables and keystones a cluster jewel can add, by name. They have no place of their own.
    pub notables: HashMap<String, TreeNode>,
    pub skills_per_orbit: Vec<usize>,
    pub orbit_radii: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Socket {
    pub name: String,
    /// 2 for the tree's large sockets, 1 and 0 for the ones a cluster adds.
    pub size: u32,
    pub index: u32,
    pub proxy: u32,
}

impl ClusterTree {
    pub fn socket(id: u32, name: &str, group: Option<u32>, e: &Value, out: &mut Self) {
        let num = |k: &str| match e.get(k) {
            Some(Value::Number(n)) => n.as_u64().map(|n| n as u32),
            Some(Value::String(s)) => s.parse().ok(),
            _ => None,
        };
        let (Some(size), Some(index), Some(proxy)) = (num("size"), num("index"), num("proxy"))
        else {
            return;
        };
        out.sockets.insert(
            id,
            Socket {
                name: name.to_string(),
                size,
                index,
                proxy,
            },
        );
        if let Some(g) = group {
            out.group_sockets.entry(g).or_default().push(id);
        }
    }
}

/// PoB's `Data/ClusterJewels.lua`: each cluster size's layout and small passives.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClusterData {
    sizes: HashMap<String, Size>,
    sort_order: HashMap<String, i64>,
    keystones: Vec<String>,
    /// The first orbit index each size starts at around a proxy, by proxy node id.
    offsets: HashMap<u32, [usize; 3]>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Size {
    name: String,
    index: u32,
    min_nodes: u32,
    max_nodes: u32,
    small: Vec<usize>,
    notable: Vec<usize>,
    socket: Vec<usize>,
    total: usize,
    skills: HashMap<String, Skill>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Skill {
    name: String,
    stats: Vec<String>,
    enchant: String,
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn indices(v: &Value) -> Vec<usize> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|n| n.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default()
}

impl ClusterData {
    pub fn from_lua(src: &str) -> Result<Self, TreeError> {
        let v = lua::parse(src)?;
        let mut out = Self::default();
        for (base, j) in v["jewels"].as_object().into_iter().flatten() {
            let n = |k: &str| j[k].as_u64().unwrap_or(0) as u32;
            let skills = j["skills"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(id, s)| {
                    (
                        id.clone(),
                        Skill {
                            name: s["name"].as_str().unwrap_or_default().to_string(),
                            stats: strings(&s["stats"]),
                            enchant: strings(&s["enchant"]).join(" ").to_lowercase(),
                        },
                    )
                })
                .collect();
            out.sizes.insert(
                base.clone(),
                Size {
                    name: j["size"].as_str().unwrap_or_default().to_string(),
                    index: n("sizeIndex"),
                    min_nodes: n("minNodes"),
                    max_nodes: n("maxNodes"),
                    small: indices(&j["smallIndicies"]),
                    notable: indices(&j["notableIndicies"]),
                    socket: indices(&j["socketIndicies"]),
                    total: n("totalIndicies") as usize,
                    skills,
                },
            );
        }
        out.sort_order = v["notableSortOrder"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(k, n)| Some((k.clone(), n.as_i64()?)))
            .collect();
        out.keystones = strings(&v["keystones"]);
        out.offsets = v["orbitOffsets"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(proxy, o)| {
                let at =
                    |i: usize| o.get(i.to_string()).and_then(Value::as_u64).unwrap_or(0) as usize;
                Some((proxy.parse().ok()?, [at(0), at(1), at(2)]))
            })
            .collect();
        Ok(out)
    }
}

/// What a cluster jewel's text says it adds (PoB's `jewelData.clusterJewel*`).
#[derive(Debug, Default)]
struct Jewel<'a> {
    size: Option<&'a Size>,
    skill: Option<&'a Skill>,
    node_count: Option<u32>,
    sockets: Option<u32>,
    socket_override: Option<u32>,
    nothingness: Option<u32>,
    smalls_are_nothing: bool,
    notables: Vec<String>,
    added: Vec<String>,
    keystone: Option<String>,
}

fn count(text: &str, prefix: &str, suffixes: &[&str]) -> Option<u32> {
    let rest = text.strip_prefix(prefix)?;
    suffixes
        .iter()
        .find_map(|s| rest.strip_suffix(s))
        .and_then(|n| n.trim().parse().ok())
}

impl<'a> Jewel<'a> {
    fn read(data: &'a ClusterData, item: &ItemText) -> Option<Self> {
        let base = item.base.as_deref().unwrap_or(&item.name);
        let size = data.sizes.get(base)?;
        let mut j = Jewel {
            size: Some(size),
            skill: item
                .cluster_skill
                .as_deref()
                .and_then(|s| size.skills.get(s)),
            node_count: item.cluster_node_count,
            ..Jewel::default()
        };
        for m in &item.mods {
            let line = m.text.trim();
            let lower = line.to_lowercase();
            if let Some(n) = count(&lower, "adds ", &[" passive skills", " passive skill"]) {
                j.node_count = j.node_count.or(Some(n));
            } else if lower == "1 added passive skill is a jewel socket" {
                j.sockets = Some(1);
            } else if let Some(n) = count(&lower, "", &[" added passive skills are jewel sockets"])
            {
                j.sockets = Some(n);
            } else if let Some(n) = count(
                &lower,
                "adds ",
                &[
                    " jewel socket passive skills",
                    " jewel socket passive skill",
                ],
            ) {
                j.socket_override = Some(n);
            } else if let Some(n) = count(
                &lower,
                "adds ",
                &[
                    " small passive skills which grant nothing",
                    " small passive skill which grants nothing",
                ],
            ) {
                j.nothingness = Some(n);
            } else if lower == "added small passive skills grant nothing" {
                j.smalls_are_nothing = true;
            } else if let Some(added) = line.strip_prefix("Added Small Passive Skills also grant: ")
            {
                j.added.push(added.to_string());
            } else if let Some(name) = line.strip_prefix("1 Added Passive Skill is ") {
                j.notables.push(name.to_string());
            } else if let Some(name) = line
                .strip_prefix("Adds ")
                .filter(|n| data.keystones.iter().any(|k| k == n))
            {
                j.keystone = Some(name.to_string());
            } else if j.skill.is_none() {
                j.skill = size.skills.values().find(|s| s.enchant == lower);
            }
        }
        if let Some(n) = j.node_count.as_mut() {
            *n = (*n).clamp(size.min_nodes, size.max_nodes);
        }
        let valid = j.keystone.is_some()
            || ((j.skill.is_some() || j.smalls_are_nothing) && j.node_count.is_some())
            || (j.socket_override.is_some() && j.nothingness.is_some());
        valid.then_some(j)
    }
}

/// The passives a build's cluster jewels add to its tree, with their links.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClusterGraph {
    pub nodes: Vec<TreeNode>,
    pub edges: Vec<TreeEdge>,
}

struct Builder<'a> {
    tree: &'a ClusterTree,
    data: &'a ClusterData,
    build: &'a PobBuild,
    spec: &'a TreeSpec,
    out: ClusterGraph,
}

pub(super) fn build(
    tree: &ClusterTree,
    data: &ClusterData,
    build: &PobBuild,
    spec: &TreeSpec,
) -> ClusterGraph {
    let mut b = Builder {
        tree,
        data,
        build,
        spec,
        out: ClusterGraph::default(),
    };
    for (socket, _) in &spec.sockets {
        if tree.sockets.get(socket).is_some_and(|s| s.size == 2) {
            b.subgraph(*socket, None);
        }
    }
    b.out
}

/// Maps a cluster's own index (6 or 12 slots around) onto the orbit's 16 slots.
fn translate(index: usize, from: usize, to: usize) -> usize {
    const TWELVE_TO_SIXTEEN: [usize; 12] = [0, 1, 3, 4, 5, 7, 8, 9, 11, 12, 13, 15];
    const SIX_TO_SIXTEEN: [usize; 6] = [0, 3, 5, 8, 11, 13];
    match (from, to) {
        _ if from == to => index,
        (12, 16) => TWELVE_TO_SIXTEEN[index % 12],
        (6, 16) => SIX_TO_SIXTEEN[index % 6],
        _ => index * to / from.max(1),
    }
}

impl Builder<'_> {
    fn jewel_at(&self, socket: u32) -> Option<ItemText> {
        let (_, item) = self.spec.sockets.iter().find(|(s, _)| *s == socket)?;
        ItemText::parse(&self.build.item(*item)?.text)
    }

    fn subgraph(&mut self, parent: u32, id: Option<u32>) {
        let (tree, data) = (self.tree, self.data);
        let Some(item) = self.jewel_at(parent) else {
            return;
        };
        let Some(jewel) = Jewel::read(data, &item) else {
            return;
        };
        let (Some(size), Some(socket)) = (jewel.size, tree.sockets.get(&parent)) else {
            return;
        };
        let Some(&group) = tree.proxies.get(&socket.proxy) else {
            return;
        };
        let Some(&(gx, gy)) = tree.groups.get(&group) else {
            return;
        };

        let mut id = id.unwrap_or(FIRST_ID);
        match socket.size {
            2 => id += socket.index << 6,
            1 => id += socket.index << 9,
            _ => {}
        }
        let base_id = id + (size.index << 4);
        let orbit = size.index as usize + 1;
        let per = tree.skills_per_orbit.get(orbit).copied().unwrap_or(1);
        let radius = tree.orbit_radii.get(orbit).copied().unwrap_or(0.);
        let start = self
            .data
            .offsets
            .get(&socket.proxy)
            .map_or(0, |o| o[(size.index as usize).min(2)]);
        let place = |index: usize| -> (f64, f64, f64) {
            let oidx = translate((index + start) % size.total.max(1), size.total, per);
            let angle = orbit_angle(per, oidx);
            (
                round1(gx + angle.sin() * radius),
                round1(gy - angle.cos() * radius),
                angle,
            )
        };

        if let Some(name) = &jewel.keystone {
            let Some(k) = tree.notables.get(name) else {
                return;
            };
            self.out.nodes.push(TreeNode {
                id: base_id,
                x: gx,
                y: gy,
                ..k.clone()
            });
            self.link(base_id, parent, None);
            return;
        }

        let mut notables: Vec<&TreeNode> = Vec::new();
        for name in &jewel.notables {
            // A notable this tree doesn't know: PoB drops the whole jewel.
            let Some(n) = tree.notables.get(name) else {
                return;
            };
            notables.push(n);
        }
        notables.sort_by_key(|n| data.sort_order.get(&n.name).copied().unwrap_or(i64::MAX));

        let socket_count = jewel.socket_override.or(jewel.sockets).unwrap_or(0) as usize;
        let notable_count = notables.len();
        let node_count = jewel.node_count.map_or(
            socket_count + notable_count + jewel.nothingness.unwrap_or(0) as usize,
            |n| n as usize,
        );
        let small_count = node_count.saturating_sub(socket_count + notable_count);
        let medium = size.name == "Medium";

        // Template index -> (node id, kind), in PoB's three passes.
        let mut slots: HashMap<usize, u32> = HashMap::new();
        let mut sockets = Vec::new();
        let socket_slots: Vec<(usize, u32)> = if size.name == "Large" && socket_count == 1 {
            vec![(6, 1)]
        } else {
            const GET_JEWELS: [u32; 3] = [0, 2, 1];
            size.socket
                .iter()
                .take(socket_count)
                .zip(GET_JEWELS)
                .map(|(i, j)| (*i, j))
                .collect()
        };
        for (index, jewel_index) in socket_slots {
            let Some(found) = tree.group_sockets.get(&group).and_then(|ids| {
                ids.iter()
                    .copied()
                    .find(|s| tree.sockets.get(s).is_some_and(|s| s.index == jewel_index))
            }) else {
                continue;
            };
            let (x, y, _) = place(index);
            let name = tree.sockets[&found].name.clone();
            self.out.nodes.push(TreeNode {
                id: found,
                x,
                y,
                kind: NodeKind::Jewel,
                name,
                ..blank()
            });
            slots.insert(index, found);
            sockets.push(found);
        }

        let mut notable_slots: Vec<usize> = Vec::new();
        for &raw in &size.notable {
            if notable_slots.len() == notable_count {
                break;
            }
            let mut i = raw;
            if medium && socket_count == 0 && notable_count == 2 {
                i = match i {
                    6 => 4,
                    10 => 8,
                    i => i,
                };
            } else if medium && node_count == 4 {
                i = match i {
                    10 => 9,
                    2 => 3,
                    i => i,
                };
            }
            if !slots.contains_key(&i) {
                notable_slots.push(i);
            }
        }
        notable_slots.sort_unstable();
        for (n, &index) in notables.iter().zip(&notable_slots) {
            let (x, y, _) = place(index);
            let node_id = base_id + index as u32;
            self.out.nodes.push(TreeNode {
                id: node_id,
                x,
                y,
                kind: NodeKind::Notable,
                ..(*n).clone()
            });
            slots.insert(index, node_id);
        }

        let mut small_slots: Vec<usize> = Vec::new();
        for &raw in &size.small {
            if small_slots.len() == small_count {
                break;
            }
            let mut i = raw;
            if medium && node_count == 5 && i == 4 {
                i = 3;
            } else if medium && node_count == 4 {
                i = match i {
                    8 => 9,
                    4 => 3,
                    i => i,
                };
            }
            if !slots.contains_key(&i) {
                small_slots.push(i);
            }
        }
        let (small_name, small_stats) = jewel
            .skill
            .map_or(("Nothingness".to_string(), Vec::new()), |s| {
                (s.name.clone(), s.stats.clone())
            });
        for &index in small_slots.iter().take(small_count) {
            let (x, y, _) = place(index);
            let node_id = base_id + index as u32;
            let mut stats = small_stats.clone();
            stats.extend(jewel.added.iter().cloned());
            self.out.nodes.push(TreeNode {
                id: node_id,
                x,
                y,
                kind: NodeKind::Normal,
                name: small_name.clone(),
                stats,
                ..blank()
            });
            slots.insert(index, node_id);
        }

        let Some(&entrance) = slots.get(&0) else {
            return;
        };
        let order: Vec<(usize, u32)> = (0..size.total)
            .filter_map(|i| slots.get(&i).map(|id| (i, *id)))
            .collect();
        for pair in order.windows(2) {
            self.link(
                pair[1].1,
                pair[0].1,
                Some((place(pair[1].0).2, place(pair[0].0).2, radius)),
            );
        }
        if let (Some(first), Some(last)) = (order.first(), order.last()) {
            if first.1 != last.1 && size.name != "Small" {
                self.link(
                    first.1,
                    last.1,
                    Some((place(first.0).2, place(last.0).2, radius)),
                );
            }
        }
        self.link(entrance, parent, None);

        for s in sockets {
            self.subgraph(s, Some(id));
        }
    }

    /// `arc` is both ends' angles and the orbit radius when they share an orbit.
    fn link(&mut self, a: u32, b: u32, arc: Option<(f64, f64, f64)>) {
        let (arc, clockwise) = match arc {
            Some((from, to, r)) => (Some(r), (to - from).rem_euclid(2. * PI) < PI),
            None => (None, false),
        };
        self.out.edges.push(TreeEdge {
            a,
            b,
            arc,
            clockwise,
        });
    }
}

fn blank() -> TreeNode {
    TreeNode {
        id: 0,
        x: 0.,
        y: 0.,
        kind: NodeKind::Normal,
        name: String::new(),
        stats: Vec::new(),
        ascendancy: None,
        mastery: Vec::new(),
        reminder: Vec::new(),
        flavour: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use exile_pob::Item;

    use super::super::TreeLayout;
    use super::*;

    const TREE: &str = r#"return {
    ["groups"]= {
        [1]= { ["x"]= 0, ["y"]= 0 },
        [2]= { ["x"]= 1000, ["y"]= 0, ["isProxy"]= true },
        [3]= { ["x"]= 1600, ["y"]= 0, ["isProxy"]= true },
    },
    ["nodes"]= {
        [100]= { ["name"]= "Large Jewel Socket", ["isJewelSocket"]= true, ["expansionJewel"]= { ["size"]= 2, ["index"]= 0, ["proxy"]= "200" }, ["group"]= 1, ["orbit"]= 0, ["orbitIndex"]= 0, ["out"]= {}, ["in"]= {} },
        [200]= { ["name"]= "Proxy", ["isProxy"]= true, ["group"]= 2, ["orbit"]= 3, ["orbitIndex"]= 0, ["out"]= {}, ["in"]= {} },
        [300]= { ["name"]= "Medium Jewel Socket", ["isJewelSocket"]= true, ["expansionJewel"]= { ["size"]= 1, ["index"]= 1, ["proxy"]= "400", ["parent"]= "100" }, ["group"]= 2, ["orbit"]= 3, ["orbitIndex"]= 4, ["out"]= {}, ["in"]= {} },
        [400]= { ["name"]= "Proxy", ["isProxy"]= true, ["group"]= 3, ["orbit"]= 2, ["orbitIndex"]= 0, ["out"]= {}, ["in"]= {} },
        [500]= { ["name"]= "Fuel the Fight", ["isNotable"]= true, ["stats"]= { "20% increased Attack Speed" }, ["orbit"]= 0, ["orbitIndex"]= 0, ["out"]= {}, ["in"]= {} },
    },
    ["constants"]= {
        ["skillsPerOrbit"]= { 1, 6, 16, 16, 40, 72, 72 },
        ["orbitRadii"]= { 0, 82, 162, 335, 493, 662, 846 },
    },
}"#;

    const DATA: &str = r#"return {
    jewels = {
        ["Large Cluster Jewel"] = {
            size = "Large", sizeIndex = 2, minNodes = 8, maxNodes = 12,
            smallIndicies = { 0, 4, 6, 8, 10, 2, 7, 5, 9, 3, 11, 1 },
            notableIndicies = { 6, 4, 8, 10, 2 },
            socketIndicies = { 4, 8, 6 },
            totalIndicies = 12,
            skills = {
                ["affliction_minion_damage"] = {
                    name = "Minion Damage",
                    stats = { "Minions deal 10% increased Damage" },
                    enchant = { "Added Small Passive Skills grant: Minions deal 10% increased Damage" },
                },
            },
        },
    },
    notableSortOrder = { ["Fuel the Fight"] = 11300 },
    keystones = { "Kineticism" },
    orbitOffsets = { [200] = { [0] = 0, [1] = 0, [2] = 0 } },
}"#;

    fn guide(jewel: &str) -> PobBuild {
        PobBuild {
            specs: vec![TreeSpec {
                sockets: vec![(100, 1)],
                ..TreeSpec::default()
            }],
            items: vec![Item {
                id: 1,
                text: jewel.into(),
            }],
            ..PobBuild::default()
        }
    }

    fn layout() -> TreeLayout {
        TreeLayout::from_lua(TREE)
            .unwrap()
            .with_cluster_data(ClusterData::from_lua(DATA).unwrap())
    }

    #[test]
    fn builds_a_large_cluster_with_pobs_node_ids() {
        let g = guide(
            "Rarity: RARE\nNew Item\nLarge Cluster Jewel\nCluster Jewel Skill: affliction_minion_damage\nImplicits: 3\n{crafted}Adds 8 Passive Skills\n{crafted}1 Added Passive Skill is a Jewel Socket\n{crafted}Added Small Passive Skills grant: Minions deal 10% increased Damage\n1 Added Passive Skill is Fuel the Fight\nAdded Small Passive Skills also grant: +5 to Strength",
        );
        let graph = layout().cluster_graph(&g, &g.specs[0]);
        // 0x10000, large socket index 0, size index 2 << 4.
        let base = 0x10000 + (2 << 4);
        let mut ids: Vec<u32> = graph.nodes.iter().map(|n| n.id).collect();
        ids.sort_unstable();
        // The one socket of a large cluster sits at index 6, the notable takes the first free
        // notable index, 4, and six smalls fill 0, 8, 10, 2, 7, 5.
        let mut want: Vec<u32> = [0, 2, 4, 5, 7, 8, 10].iter().map(|i| base + i).collect();
        want.push(300);
        want.sort_unstable();
        assert_eq!(ids, want);
        let notable = graph.nodes.iter().find(|n| n.id == base + 4).unwrap();
        assert_eq!(notable.name, "Fuel the Fight");
        assert_eq!(notable.kind, NodeKind::Notable);
        let small = graph.nodes.iter().find(|n| n.id == base).unwrap();
        assert_eq!(
            small.stats,
            vec!["Minions deal 10% increased Damage", "+5 to Strength"]
        );
        // Index 0 is the top of the proxy group's orbit 3.
        assert_eq!((small.x, small.y), (1000., -335.));
        // A ring of 8 is 7 links and the one closing it, then the way in from the socket.
        assert_eq!(graph.edges.len(), 9);
        assert!(graph
            .edges
            .iter()
            .any(|e| (e.a, e.b) == (base, 100) && e.arc.is_none()));
        assert!(graph.edges.iter().filter(|e| e.arc == Some(335.)).count() == 8);
    }

    #[test]
    fn a_jewel_that_isnt_a_cluster_adds_nothing() {
        let g = guide("Rarity: RARE\nNew Item\nCobalt Jewel\nImplicits: 0\n+10 to Strength");
        assert!(layout().cluster_graph(&g, &g.specs[0]).nodes.is_empty());
        let unknown = guide("Rarity: RARE\nNew Item\nLarge Cluster Jewel\nCluster Jewel Skill: affliction_minion_damage\nImplicits: 1\nAdds 8 Passive Skills\n1 Added Passive Skill is Not A Notable");
        assert!(layout()
            .cluster_graph(&unknown, &unknown.specs[0])
            .nodes
            .is_empty());
    }

    #[test]
    fn maps_a_twelve_slot_ring_onto_sixteen() {
        assert_eq!(translate(0, 12, 16), 0);
        assert_eq!(translate(2, 12, 16), 3);
        assert_eq!(translate(11, 12, 16), 15);
        assert_eq!(translate(3, 6, 6), 3);
    }
}
