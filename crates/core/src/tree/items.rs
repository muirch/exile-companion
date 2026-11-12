//! What a build's items do to its tree: notables anointed on the amulet, tattoos, and jewels with
//! the area they reach.

use exile_pob::{ItemSet, ItemText, PobBuild, TreeSpec};
use serde::Serialize;

use super::{NodeKind, TreeLayout};
use crate::gear::ItemView;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tattoo {
    pub name: String,
    pub stats: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JewelMark {
    pub socket: u32,
    pub item: ItemView,
    /// The node the jewel's area is around: its socket, or for Impossible Escape the keystone
    /// it names.
    pub centre: u32,
    /// In tree units. None for a jewel without an area, or a ring (Thread of Hope).
    pub radius: Option<f64>,
    /// Nodes in the area can be taken without a path to them (Impossible Escape, Intuitive Leap).
    pub without_path: bool,
}

/// A jewel's radius by the label PoB gives it, for a tree version. The radii grew in 3.16.
pub fn jewel_radius(label: &str, version: Option<&str>) -> Option<f64> {
    let old = version
        .and_then(|v| {
            let (major, minor) = v.split_once('_')?;
            let minor: u32 = minor.split('_').next()?.parse().ok()?;
            Some(major == "3" && minor <= 15)
        })
        .unwrap_or(false);
    Some(match (label, old) {
        ("Small", false) => 960.,
        ("Medium", false) => 1440.,
        ("Large", false) => 1800.,
        ("Very Large", false) => 2400.,
        ("Massive", false) => 2880.,
        ("Small", true) => 800.,
        ("Medium", true) => 1200.,
        ("Large", true) => 1500.,
        _ => return None,
    })
}

/// The gear set a build wears now: the active one, else the first.
pub fn active_set(build: &PobBuild) -> Option<&ItemSet> {
    build
        .item_sets
        .iter()
        .find(|s| s.id.is_some() && s.id == build.active_item_set)
        .or_else(|| build.item_sets.first())
}

/// The notables the amulet in `set` anoints.
pub fn anointed(layout: &TreeLayout, build: &PobBuild, set: Option<&ItemSet>) -> Vec<u32> {
    let Some(amulet) = set
        .and_then(|s| s.slots.iter().find(|(slot, _)| slot == "Amulet"))
        .and_then(|(_, id)| build.item(*id))
        .and_then(|i| ItemText::parse(&i.text))
    else {
        return Vec::new();
    };
    amulet
        .allocates()
        .filter_map(|name| {
            layout
                .nodes
                .iter()
                .find(|n| n.kind == NodeKind::Notable && n.ascendancy.is_none() && n.name == name)
                .map(|n| n.id)
        })
        .collect()
}

pub fn tattoo(spec: Option<&TreeSpec>, node: u32) -> Option<Tattoo> {
    spec?
        .overrides
        .iter()
        .find(|o| o.node == node)
        .map(|o| Tattoo {
            name: o.name.clone(),
            stats: o.stats.clone(),
        })
}

pub fn jewels(layout: &TreeLayout, build: &PobBuild, spec: Option<&TreeSpec>) -> Vec<JewelMark> {
    let Some(spec) = spec else { return Vec::new() };
    let version = spec.tree_version.as_deref();
    spec.sockets
        .iter()
        .filter(|(socket, _)| layout.node(*socket).is_some())
        .filter_map(|(socket, id)| {
            let text = ItemText::parse(&build.item(*id)?.text)?;
            let escape = text.mods.iter().find_map(|m| {
                let rest = m.text.strip_prefix("Passive Skills in radius of ")?;
                rest.strip_suffix(" can be allocated without being connected to your tree")
            });
            let centre = escape
                .and_then(|name| {
                    layout
                        .nodes
                        .iter()
                        .find(|n| n.kind == NodeKind::Keystone && n.name == name)
                })
                .map_or(*socket, |n| n.id);
            let without_path = escape.is_some()
                || text.mods.iter().any(|m| {
                    m.text.eq_ignore_ascii_case(
                        "Passives in Radius can be Allocated without being connected to your tree",
                    )
                });
            Some(JewelMark {
                socket: *socket,
                radius: text
                    .radius
                    .as_deref()
                    .and_then(|r| jewel_radius(r, version)),
                item: ItemView::from(&text),
                centre,
                without_path,
            })
        })
        .collect()
}
