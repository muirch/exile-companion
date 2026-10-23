//! Leveling routes in exile-leveling's text format, and a tracker that follows the player along one.
//!
//! Format reference: <https://github.com/HeartofPhos/exile-leveling/tree/main/common/data/routes>.
//! Every step is pinned to the area the player is in while doing it. Each time that area changes
//! the route gets a new "edge", and the tracker moves one edge forward when the game generates the
//! area the next edge expects. That mirrors exile-leveling's own auto-progress.

use std::collections::{HashMap, HashSet};

use crate::data::{QuestTable, ZoneTable};

const START_AREA: &str = "1_1_1";
const START_TOWN: &str = "1_1_town";
const LAB_AIRLOCK: &str = "Labyrinth_Airlock";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Part {
    Text {
        text: String,
    },
    Kill {
        name: String,
    },
    Arena {
        name: String,
    },
    Area {
        id: String,
        name: String,
    },
    Enter {
        id: String,
        name: String,
    },
    Logout {
        town: String,
    },
    Waypoint,
    WaypointUse {
        id: String,
        name: String,
    },
    WaypointGet,
    PortalSet,
    PortalUse {
        name: String,
    },
    Quest {
        id: String,
        name: String,
    },
    QuestText {
        text: String,
    },
    Generic {
        text: String,
    },
    RewardQuest {
        item: String,
    },
    RewardVendor {
        item: String,
        cost: Option<String>,
    },
    Trial,
    Ascend {
        difficulty: String,
    },
    Crafting,
    /// Compass direction in degrees, a multiple of 45.
    Dir {
        degrees: u16,
    },
    Copy {
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
pub struct Step {
    pub parts: Vec<Part>,
    pub sub_steps: Vec<Vec<Part>>,
    #[serde(skip)]
    pub edge: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub sections: Vec<Section>,
    /// Area id per edge. `edges[0]` is where the character spawns.
    pub edges: Vec<String>,
    /// Monster level the player is heading into at each edge: the area's own level, or for towns
    /// the next combat area's. Tells the many visits to one town apart.
    pub edge_levels: Vec<Option<u32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteWarning {
    pub line: usize,
    pub message: String,
}

struct Parser<'a> {
    zones: &'a ZoneTable,
    quests: &'a QuestTable,
    defines: &'a HashSet<String>,
    current_area: String,
    last_town: String,
    portal_area: Option<String>,
    names_from_comments: HashMap<String, String>,
    warnings: Vec<RouteWarning>,
    line: usize,
}

/// Parses a whole route (all act files joined in order). `defines` switches `#ifdef` blocks, e.g.
/// `LEAGUE_START`. Problems don't stop parsing. They come back as warnings.
pub fn parse_route(
    source: &str,
    zones: &ZoneTable,
    quests: &QuestTable,
    defines: &HashSet<String>,
) -> (Route, Vec<RouteWarning>) {
    let mut p = Parser {
        zones,
        quests,
        defines,
        current_area: START_AREA.to_string(),
        last_town: START_TOWN.to_string(),
        portal_area: None,
        names_from_comments: HashMap::new(),
        warnings: Vec::new(),
        line: 0,
    };
    let mut route = Route {
        sections: Vec::new(),
        edges: vec![START_AREA.to_string()],
        edge_levels: Vec::new(),
    };
    let mut conditions: Vec<bool> = Vec::new();
    let mut prev_area: Option<String> = None;

    for (i, raw) in source.lines().enumerate() {
        p.line = i + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(name) = line.strip_prefix("#section") {
            route.sections.push(Section {
                name: name.trim().to_string(),
                steps: Vec::new(),
            });
            continue;
        }
        if line.starts_with("#endif") {
            if conditions.pop().is_none() {
                p.warn("#endif without #ifdef");
            }
            continue;
        }
        if let Some(def) = line.strip_prefix("#ifdef") {
            conditions.push(p.defines.contains(def.trim()));
            continue;
        }
        if let Some(def) = line.strip_prefix("#ifndef") {
            conditions.push(!p.defines.contains(def.trim()));
            continue;
        }
        if !conditions.iter().all(|&c| c) {
            continue;
        }

        if route.sections.is_empty() {
            route.sections.push(Section {
                name: "Default".to_string(),
                steps: Vec::new(),
            });
        }
        let last = route.sections.len() - 1;
        let steps = &mut route.sections[last].steps;

        if let Some(sub) = line.strip_prefix("#sub") {
            let parts = p.parse_line(sub.trim());
            match steps.last_mut() {
                Some(step) if !parts.is_empty() => step.sub_steps.push(parts),
                Some(_) => {}
                None => p.warn("#sub before any step"),
            }
            continue;
        }

        let parts = p.parse_line(line);
        if parts.is_empty() {
            continue;
        }
        let mut edge = None;
        match &prev_area {
            None => edge = Some(0),
            Some(prev) if *prev != p.current_area => {
                edge = Some(route.edges.len());
                route.edges.push(p.current_area.clone());
            }
            Some(_) => {}
        }
        prev_area = Some(p.current_area.clone());
        steps.push(Step {
            parts,
            sub_steps: Vec::new(),
            edge,
        });
    }

    if !conditions.is_empty() {
        p.warn("missing #endif");
    }
    route.edge_levels = edge_levels(&route.edges, zones);
    (route, p.warnings)
}

impl Parser<'_> {
    fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(RouteWarning {
            line: self.line,
            message: message.into(),
        });
    }

    fn parse_line(&mut self, line: &str) -> Vec<Part> {
        // A `#` outside braces starts a comment. Authors put the area or quest name there, which
        // is the only name we have for areas missing from the pack (the labyrinth ones).
        let (body, comment) = split_comment(line);
        let mut parts = Vec::new();
        let mut rest = body;
        while !rest.is_empty() {
            match rest.find('{') {
                Some(0) => {
                    let Some(end) = rest.find('}') else {
                        self.warn("unclosed {");
                        break;
                    };
                    if let Some(part) = self.fragment(&rest[1..end], comment) {
                        parts.push(part);
                    }
                    rest = &rest[end + 1..];
                }
                Some(start) => {
                    parts.push(Part::Text {
                        text: rest[..start].to_string(),
                    });
                    rest = &rest[start..];
                }
                None => {
                    parts.push(Part::Text {
                        text: rest.to_string(),
                    });
                    break;
                }
            }
        }
        if let Some(Part::Text { text }) = parts.last_mut() {
            let trimmed = text.trim_end().len();
            text.truncate(trimmed);
        }
        parts
    }

    fn fragment(&mut self, raw: &str, comment: Option<&str>) -> Option<Part> {
        let args: Vec<&str> = raw.split('|').collect();
        let part = match args.as_slice() {
            ["kill", name] => Part::Kill {
                name: name.to_string(),
            },
            ["arena", name] => Part::Arena {
                name: name.to_string(),
            },
            ["area", id] => Part::Area {
                id: id.to_string(),
                name: self.area_name(id, comment),
            },
            ["enter", id] => {
                let name = self.area_name(id, comment);
                self.transition(id, comment);
                Part::Enter {
                    id: id.to_string(),
                    name,
                }
            }
            ["logout"] => {
                let town = self.last_town.clone();
                self.transition(&town, None);
                self.portal_area = None;
                Part::Logout {
                    town: self.area_name(&town, None),
                }
            }
            ["waypoint"] => Part::Waypoint,
            ["waypoint", id] => {
                let name = self.area_name(id, comment);
                self.transition(id, comment);
                Part::WaypointUse {
                    id: id.to_string(),
                    name,
                }
            }
            ["waypoint_get"] => Part::WaypointGet,
            ["portal", "set"] => {
                self.portal_area = Some(self.current_area.clone());
                Part::PortalSet
            }
            ["portal", "use"] => self.use_portal()?,
            ["quest", id, ..] => {
                let name = match self.quests.by_id(id) {
                    Some(q) => q.name.clone(),
                    None => {
                        self.warn(format!("unknown quest {id}"));
                        comment.unwrap_or(id).to_string()
                    }
                };
                Part::Quest {
                    id: id.to_string(),
                    name,
                }
            }
            ["quest_text", text] => Part::QuestText {
                text: text.to_string(),
            },
            ["generic", text] => Part::Generic {
                text: text.to_string(),
            },
            ["reward_quest", item] => Part::RewardQuest {
                item: item.to_string(),
            },
            ["reward_vendor", item] => Part::RewardVendor {
                item: item.to_string(),
                cost: None,
            },
            ["reward_vendor", item, cost] => Part::RewardVendor {
                item: item.to_string(),
                cost: Some(cost.to_string()),
            },
            ["trial"] => Part::Trial,
            ["ascend", difficulty] => {
                if self.current_area != LAB_AIRLOCK {
                    self.warn("ascend outside the labyrinth airlock");
                }
                let town = self.last_town.clone();
                self.transition(&town, None);
                Part::Ascend {
                    difficulty: difficulty.to_string(),
                }
            }
            ["crafting"] | ["crafting", _] => Part::Crafting,
            ["dir", degrees] => match degrees.parse::<u16>() {
                Ok(d) if d % 45 == 0 && d < 360 => Part::Dir { degrees: d },
                _ => {
                    self.warn(format!("bad direction {degrees}"));
                    return None;
                }
            },
            ["copy", rest @ ..] if !rest.is_empty() => Part::Copy {
                text: rest.concat(),
            },
            _ => {
                self.warn(format!("unknown fragment {{{raw}}}"));
                return None;
            }
        };
        Some(part)
    }

    fn use_portal(&mut self) -> Option<Part> {
        let current_is_town = self.is_town(&self.current_area);
        if !current_is_town && self.portal_area.as_deref() != Some(&self.current_area) {
            self.portal_area = Some(self.current_area.clone());
        }
        let Some(portal) = self.portal_area.clone() else {
            self.warn("portal used before it was set");
            return None;
        };
        let portal_town = self.parent_town(&portal);
        let dest = if self.current_area == portal {
            let Some(town) = portal_town else {
                self.warn(format!("no town for portal area {portal}"));
                return None;
            };
            town
        } else if portal_town.as_deref() == Some(self.current_area.as_str()) {
            self.portal_area = None;
            portal
        } else {
            self.warn("portal used away from both its town and its area");
            return None;
        };
        self.transition(&dest, None);
        Some(Part::PortalUse {
            name: self.area_name(&dest, None),
        })
    }

    fn transition(&mut self, id: &str, comment: Option<&str>) {
        // The pack only has campaign areas. Lab rooms and Karui Shores are fine as long as the
        // route names them in a comment at least once.
        if self.zones.by_id(id).is_none() {
            match comment {
                Some(name) => {
                    self.names_from_comments
                        .insert(id.to_string(), name.to_string());
                }
                None if !self.names_from_comments.contains_key(id) => {
                    self.warn(format!("area {id} is not in the data pack and has no name"));
                }
                None => {}
            }
        }
        if self.is_town(id) {
            self.last_town = id.to_string();
        }
        self.current_area = id.to_string();
    }

    fn is_town(&self, id: &str) -> bool {
        self.zones.by_id(id).is_some_and(|z| z.is_town)
    }

    fn parent_town(&self, id: &str) -> Option<String> {
        let act = self.zones.by_id(id)?.act;
        self.zones.town_of_act(act).map(|z| z.id.clone())
    }

    fn area_name(&self, id: &str, comment: Option<&str>) -> String {
        match (self.zones.by_id(id), comment) {
            (Some(z), _) => z.name.clone(),
            (None, Some(c)) => c.to_string(),
            (None, None) => self
                .names_from_comments
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.to_string()),
        }
    }
}

fn edge_levels(edges: &[String], zones: &ZoneTable) -> Vec<Option<u32>> {
    let own: Vec<Option<u32>> = edges
        .iter()
        .map(|id| {
            zones
                .by_id(id)
                .filter(|z| !z.is_town)
                .map(|z| z.area_level.max(0) as u32)
        })
        .collect();
    let mut out = vec![None; edges.len()];
    let mut ahead = None;
    for i in (0..edges.len()).rev() {
        if own[i].is_some() {
            ahead = own[i];
        }
        out[i] = own[i].or(ahead);
    }
    out
}

fn is_move(part: &Part) -> bool {
    matches!(
        part,
        Part::Enter { .. }
            | Part::WaypointUse { .. }
            | Part::Logout { .. }
            | Part::PortalUse { .. }
            | Part::Ascend { .. }
    )
}

// Cuts everything up to the last move in the step, plus the ", " that joined it to the rest.
fn drop_arrival(step: &mut Step) {
    let Some(last_move) = step.parts.iter().rposition(is_move) else {
        return;
    };
    step.parts.drain(..=last_move);
    if let Some(Part::Text { text }) = step.parts.first_mut() {
        let trimmed = text.trim_start_matches([',', ' ']);
        if trimmed.is_empty() {
            step.parts.remove(0);
        } else {
            let mut t = trimmed.to_string();
            if let Some(c) = t.get(..1) {
                t.replace_range(..1, &c.to_uppercase());
            }
            *text = t;
        }
    }
}

fn split_comment(line: &str) -> (&str, Option<&str>) {
    let mut depth = 0u32;
    for (i, c) in line.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '#' if depth == 0 => {
                let comment = line[i + 1..].trim();
                return (
                    line[..i].trim_end(),
                    (!comment.is_empty()).then_some(comment),
                );
            }
            _ => {}
        }
    }
    (line, None)
}

/// What the overlay shows: the steps for the area the player should be in now, plus the next group
/// as a preview.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
pub struct RouteView {
    pub section: String,
    pub edge: u32,
    pub edge_count: u32,
    /// Position of `edge` among the edges of its section, and how many that section has.
    pub section_edge: u32,
    pub section_edge_count: u32,
    /// The section the player just left, on the first edge of the next one ("Act 1" on the
    /// first step of act 2).
    pub finished_section: Option<String>,
    /// The area this edge expects the player to be in.
    pub area_id: String,
    /// What to do in this area. The move that brought the player here is already done, so it is
    /// cut from the first step.
    pub here: Vec<Step>,
    /// The step that leaves for the next area.
    pub then: Option<Step>,
}

impl Route {
    fn locate_edge(&self, edge: usize) -> Option<(usize, usize)> {
        self.sections.iter().enumerate().find_map(|(si, s)| {
            s.steps
                .iter()
                .position(|st| st.edge == Some(edge))
                .map(|pi| (si, pi))
        })
    }

    fn group(&self, edge: usize) -> Option<(&str, Vec<Step>)> {
        let (si, pi) = self.locate_edge(edge)?;
        let section = &self.sections[si];
        let mut steps = vec![section.steps[pi].clone()];
        steps.extend(
            section.steps[pi + 1..]
                .iter()
                .take_while(|s| s.edge.is_none())
                .cloned(),
        );
        Some((&section.name, steps))
    }

    pub fn view(&self, edge: usize) -> Option<RouteView> {
        let (section, mut here) = self.group(edge)?;
        if edge > 0 {
            if let Some(first) = here.first_mut() {
                drop_arrival(first);
            }
            if here.first().is_some_and(|s| s.parts.is_empty()) {
                let rest = here.remove(0);
                // Hints under the arrival line ("Go right of the bridge") still apply here.
                if let Some(next) = here.first_mut() {
                    next.sub_steps.splice(0..0, rest.sub_steps);
                } else if !rest.sub_steps.is_empty() {
                    here.push(Step {
                        parts: Vec::new(),
                        sub_steps: rest.sub_steps,
                        edge: None,
                    });
                }
            }
        }
        let then = self.group(edge + 1).and_then(|(_, s)| s.into_iter().next());
        let (si, _) = self.locate_edge(edge)?;
        let section_edges: Vec<usize> = self.sections[si]
            .steps
            .iter()
            .filter_map(|s| s.edge)
            .collect();
        let section_edge = section_edges.iter().position(|&e| e == edge).unwrap_or(0);
        let finished_section =
            (section_edge == 0 && si > 0).then(|| self.sections[si - 1].name.clone());
        Some(RouteView {
            section: section.to_string(),
            edge: edge as u32,
            edge_count: self.edges.len() as u32,
            section_edge: section_edge as u32,
            section_edge_count: section_edges.len() as u32,
            finished_section,
            area_id: self.edges.get(edge).cloned().unwrap_or_default(),
            here,
            then,
        })
    }
}

// Players run a few levels over the zone. Past this the nearest visit is probably the wrong one.
const LEVEL_SLACK: u32 = 5;

#[derive(Debug, Clone, Default)]
pub struct RouteTracker {
    active: usize,
}

impl RouteTracker {
    pub fn at(edge: usize) -> Self {
        Self { active: edge }
    }

    pub fn active(&self) -> usize {
        self.active
    }

    /// Feed the area id from a `Generating level .. area "<id>"` line. The expected next area
    /// steps forward. Any other area on the route snaps to its nearest edge, so a character that
    /// is further along (or went back to town) lands on the right steps. Areas off the route
    /// (hideouts, maps) leave the position alone.
    pub fn on_area_generated(&mut self, route: &Route, area_id: &str) -> bool {
        self.on_area(route, area_id, None)
    }

    /// Like [`Self::on_area_generated`], but when the nearest match is far from the character's
    /// level it picks the visit whose level fits instead. A level 90 character standing in
    /// Lioneye's Watch belongs at the last act 1 town visit, not the first.
    pub fn on_area(&mut self, route: &Route, area_id: &str, char_level: Option<u32>) -> bool {
        let level_gap = |i: usize| match (route.edge_levels.get(i).copied().flatten(), char_level) {
            (Some(edge), Some(ch)) => edge.abs_diff(ch),
            _ => 0,
        };
        let fits = |i: usize| {
            route.edges.get(i).is_some_and(|e| e == area_id) && level_gap(i) <= LEVEL_SLACK
        };
        if fits(self.active + 1) {
            self.active += 1;
            return true;
        }
        if fits(self.active) {
            return false;
        }
        let candidates: Vec<usize> = route
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| *e == area_id)
            .map(|(i, _)| i)
            .collect();
        let nearest = candidates
            .iter()
            .copied()
            .min_by_key(|&i| (i.abs_diff(self.active), i < self.active));
        let pick = match nearest {
            Some(i) if level_gap(i) > LEVEL_SLACK => candidates
                .iter()
                .copied()
                .min_by_key(|&c| (level_gap(c), c.abs_diff(self.active))),
            other => other,
        };
        match pick {
            Some(i) if i != self.active => {
                self.active = i;
                true
            }
            _ => false,
        }
    }

    pub fn next(&mut self, route: &Route) {
        if self.active + 1 < route.edges.len() {
            self.active += 1;
        }
    }

    pub fn prev(&mut self) {
        self.active = self.active.saturating_sub(1);
    }

    pub fn reset(&mut self) {
        self.active = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREAS: &str = r#"{"schema_version":1,"patch":"t","generated_at":"t","source":"t","areas":[
        {"id":"1_1_1","name":"The Twilight Strand","act":1,"area_level":1,"is_town":false,"is_hideout":false,"is_map":false,"has_waypoint":false,"connections":["1_1_town"]},
        {"id":"1_1_town","name":"Lioneye's Watch","act":1,"area_level":1,"is_town":true,"is_hideout":false,"is_map":false,"has_waypoint":true,"connections":["1_1_1","1_1_2"]},
        {"id":"1_1_2","name":"The Coast","act":1,"area_level":2,"is_town":false,"is_hideout":false,"is_map":false,"has_waypoint":true,"connections":["1_1_town","1_1_2a"]},
        {"id":"1_1_2a","name":"The Tidal Island","act":1,"area_level":3,"is_town":false,"is_hideout":false,"is_map":false,"has_waypoint":false,"connections":["1_1_2"]}
    ]}"#;
    const QUESTS: &str = r#"{"schema_version":1,"quests":[
        {"id":"a1q1","act":1,"name":"Enemy at the Gate","npc":"Tarkleigh","passive_points":0,"gem_rewards":[]}
    ]}"#;

    fn parse(src: &str, defines: &[&str]) -> (Route, Vec<RouteWarning>) {
        let zones = ZoneTable::from_json_bytes(AREAS.as_bytes(), 1).unwrap();
        let quests = QuestTable::from_json_bytes(QUESTS.as_bytes(), 1).unwrap();
        let defines = defines.iter().map(|d| d.to_string()).collect();
        parse_route(src, &zones, &quests, &defines)
    }

    const ROUTE: &str = "#section Act 1
Find and kill {kill|Hillock}
➞ {enter|1_1_town} #Lioneye's Watch
Hand in {quest|a1q1} #Enemy at the Gate
➞ {enter|1_1_2}
#ifdef LEAGUE_START
    Get {waypoint_get}
#endif
➞ {enter|1_1_2a}, place {portal|set}
    #sub Go {dir|270}
Take {portal|use}
{logout}
";

    #[test]
    fn steps_get_edges_when_the_area_changes() {
        let (route, warnings) = parse(ROUTE, &[]);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            route.edges,
            ["1_1_1", "1_1_town", "1_1_2", "1_1_2a", "1_1_town"]
        );
        let edges: Vec<_> = route.sections[0].steps.iter().map(|s| s.edge).collect();
        assert_eq!(
            edges,
            [Some(0), Some(1), None, Some(2), Some(3), Some(4), None]
        );
    }

    #[test]
    fn names_come_from_the_pack() {
        let (route, _) = parse(ROUTE, &[]);
        let step = &route.sections[0].steps[2];
        assert_eq!(
            step.parts,
            [
                Part::Text {
                    text: "Hand in ".into()
                },
                Part::Quest {
                    id: "a1q1".into(),
                    name: "Enemy at the Gate".into()
                },
            ]
        );
    }

    #[test]
    fn ifdef_and_sub_steps() {
        let (route, _) = parse(ROUTE, &["LEAGUE_START"]);
        let steps = &route.sections[0].steps;
        assert!(steps[4].parts.contains(&Part::WaypointGet));
        let portal_step = &steps[5];
        assert_eq!(portal_step.sub_steps[0][1], Part::Dir { degrees: 270 });
    }

    #[test]
    fn unknown_fragments_are_reported() {
        let (_, warnings) = parse("{teleport|home}\n", &[]);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].line, 1);
    }

    // edges: 0 1_1_1, 1 1_1_town, 2 1_1_2, 3 1_1_2a, 4 1_1_town
    #[test]
    fn tracker_steps_forward_on_the_expected_area() {
        let (route, _) = parse(ROUTE, &[]);
        let mut t = RouteTracker::default();
        assert!(t.on_area_generated(&route, "1_1_town"));
        assert!(t.on_area_generated(&route, "1_1_2"));
        assert_eq!(t.active(), 2);
        assert!(!t.on_area_generated(&route, "1_1_2"));
    }

    #[test]
    fn tracker_snaps_to_the_nearest_matching_area() {
        let (route, _) = parse(ROUTE, &[]);
        let mut t = RouteTracker::default();
        assert!(t.on_area_generated(&route, "1_1_2a"));
        assert_eq!(t.active(), 3);
        // a trip to town picks the closer town edge, and coming back restores the zone
        assert!(t.on_area_generated(&route, "1_1_town"));
        assert_eq!(t.active(), 4);
        assert!(t.on_area_generated(&route, "1_1_2a"));
        assert_eq!(t.active(), 3);
        assert!(!t.on_area_generated(&route, "HideoutFelled"));
        assert_eq!(t.active(), 3);
    }

    #[test]
    fn character_level_picks_between_town_visits() {
        let (route, _) = parse(ROUTE, &[]);
        assert_eq!(
            route.edge_levels,
            [Some(1), Some(2), Some(2), Some(3), None]
        );
        let mut t = RouteTracker::default();
        assert!(t.on_area(&route, "1_1_town", Some(2)));
        assert_eq!(t.active(), 1);
        let mut t = RouteTracker::at(0);
        assert!(t.on_area(&route, "1_1_town", Some(40)));
        assert_eq!(t.active(), 4);
    }

    #[test]
    fn steps_are_counted_within_their_section() {
        let src = "#section Act 1
Find and kill {kill|Hillock}
➞ {enter|1_1_town}
#section Act 2
➞ {enter|1_1_2}
➞ {enter|1_1_2a}
";
        let (route, _) = parse(src, &[]);
        let counts: Vec<_> = (0..4)
            .map(|e| {
                let v = route.view(e).unwrap();
                (v.section, v.section_edge, v.section_edge_count)
            })
            .collect();
        assert_eq!(
            counts,
            [
                ("Act 1".to_string(), 0, 2),
                ("Act 1".to_string(), 1, 2),
                ("Act 2".to_string(), 0, 2),
                ("Act 2".to_string(), 1, 2),
            ]
        );
        let finished: Vec<_> = (0..4)
            .map(|e| route.view(e).unwrap().finished_section)
            .collect();
        assert_eq!(finished, [None, None, Some("Act 1".to_string()), None]);
    }

    #[test]
    fn view_drops_the_arrival_and_names_the_way_out() {
        let (route, _) = parse(ROUTE, &[]);
        let view = route.view(1).unwrap();
        assert_eq!(view.section, "Act 1");
        assert_eq!(view.section_edge, 1);
        assert_eq!(view.section_edge_count, view.edge_count);
        // "➞ Lioneye's Watch" is how the player got here. Only the hand-in remains
        assert_eq!(view.here.len(), 1);
        assert!(matches!(view.here[0].parts[1], Part::Quest { .. }));
        let then = view.then.unwrap();
        assert!(then
            .parts
            .iter()
            .any(|p| matches!(p, Part::Enter { id, .. } if id == "1_1_2")));

        // "➞ The Tidal Island, place portal" keeps "Place portal"
        let view = route.view(3).unwrap();
        assert_eq!(
            view.here[0].parts,
            [
                Part::Text {
                    text: "Place ".into()
                },
                Part::PortalSet
            ]
        );
        assert_eq!(view.here[0].sub_steps[0][1], Part::Dir { degrees: 270 });
    }
}
