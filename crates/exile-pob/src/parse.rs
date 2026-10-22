//! Parse Path of Building XML into a [`PobBuild`].
//!
//! A streaming quick-xml walk. Children are matched BY NAME (document order is not guaranteed) and
//! every attribute is optional. We only read the elements we care about (`Build`, `Tree`/`Spec`,
//! `Skills`/`SkillSet`/`Skill`/`Gem`) and ignore the rest (`Calcs`, `PlayerStat`, `Notes`, `URL`,
//! `Sockets`, ...). Both `<Skill>`-directly-under-`<Skills>` (legacy) and the newer `<SkillSet>`
//! grouping are handled.

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use crate::error::PobError;
use crate::model::{Gem, PobBuild, SkillGroup, SkillSet, TreeSpec};

/// Parse PoB XML bytes into a build. Errors only on malformed XML or a non-PoB root.
pub fn parse_xml(xml: &[u8]) -> Result<PobBuild, PobError> {
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut p = Parser::default();
    let mut root_seen = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                if !root_seen {
                    root_seen = true;
                    require_root(&e)?;
                    continue;
                }
                p.open(&e, false);
            }
            Event::Empty(e) => {
                if !root_seen {
                    root_seen = true;
                    require_root(&e)?;
                    continue;
                }
                p.open(&e, true);
            }
            Event::End(e) => p.close(local_name(e.name().as_ref())),
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(p.finish())
}

fn require_root(e: &BytesStart<'_>) -> Result<(), PobError> {
    let name = local_name(e.name().as_ref());
    if name == "PathOfBuilding" {
        Ok(())
    } else {
        Err(PobError::NotPob { found: name })
    }
}

/// Accumulating parse state. `build` starts from `PobBuild::default()` (active_spec = 1).
#[derive(Default)]
struct Parser {
    build: PobBuild,
    cur_spec: Option<TreeSpec>,
    cur_skillset: Option<SkillSet>,
    cur_group: Option<SkillGroup>,
    /// Skills written directly under `<Skills>` (no enclosing `<SkillSet>`).
    legacy: SkillSet,
}

impl Parser {
    /// Handle a Start or Empty element. `empty` marks self-closing elements, which we finalize at once.
    fn open(&mut self, e: &BytesStart<'_>, empty: bool) {
        match local_name(e.name().as_ref()).as_str() {
            "Build" => self.read_build(e),
            "Tree" => self.build.active_spec = attr_usize(e, "activeSpec").unwrap_or(1).max(1),
            "Spec" => {
                let spec = read_spec(e);
                if empty {
                    self.build.specs.push(spec);
                } else {
                    self.cur_spec = Some(spec);
                }
            }
            "Skills" => self.build.active_skill_set_id = attr_u32(e, "activeSkillSet"),
            "SkillSet" => {
                let set = read_skillset(e);
                if empty {
                    self.build.skill_sets.push(set);
                } else {
                    self.cur_skillset = Some(set);
                }
            }
            "Skill" => {
                let group = read_group(e);
                if empty {
                    self.push_group(group);
                } else {
                    self.cur_group = Some(group);
                }
            }
            "Gem" => {
                if let Some(group) = self.cur_group.as_mut() {
                    group.gems.push(read_gem(e));
                }
            }
            _ => {}
        }
    }

    fn close(&mut self, name: String) {
        match name.as_str() {
            "Spec" => {
                if let Some(spec) = self.cur_spec.take() {
                    self.build.specs.push(spec);
                }
            }
            "SkillSet" => {
                if let Some(set) = self.cur_skillset.take() {
                    self.build.skill_sets.push(set);
                }
            }
            "Skill" => {
                if let Some(group) = self.cur_group.take() {
                    self.push_group(group);
                }
            }
            _ => {}
        }
    }

    /// Route a finished socket group into its enclosing skill set, or the legacy synthetic set.
    fn push_group(&mut self, group: SkillGroup) {
        match self.cur_skillset.as_mut() {
            Some(set) => set.groups.push(group),
            None => self.legacy.groups.push(group),
        }
    }

    fn read_build(&mut self, e: &BytesStart<'_>) {
        self.build.level = attr_u32(e, "level");
        self.build.class_name = attr_string(e, "className");
        self.build.ascend_class_name = attr_string(e, "ascendClassName").filter(is_meaningful);
        self.build.main_socket_group = attr_u32(e, "mainSocketGroup");
        self.build.target_version = attr_string(e, "targetVersion");
    }

    fn finish(mut self) -> PobBuild {
        // A build with skills written directly under <Skills> yields a single synthetic set.
        if !self.legacy.groups.is_empty() {
            self.build.skill_sets.push(self.legacy);
        }
        self.build
    }
}

fn read_spec(e: &BytesStart<'_>) -> TreeSpec {
    TreeSpec {
        title: attr_string(e, "title"),
        tree_version: attr_string(e, "treeVersion"),
        class_id: attr_u8(e, "classId"),
        ascend_class_id: attr_u8(e, "ascendClassId"),
        nodes: attr_string(e, "nodes").map(parse_nodes).unwrap_or_default(),
    }
}

fn read_skillset(e: &BytesStart<'_>) -> SkillSet {
    SkillSet {
        id: attr_u32(e, "id"),
        title: attr_string(e, "title"),
        groups: Vec::new(),
    }
}

fn read_group(e: &BytesStart<'_>) -> SkillGroup {
    SkillGroup {
        enabled: attr_bool(e, "enabled").unwrap_or(true),
        slot: attr_string(e, "slot").filter(|s| !s.is_empty()),
        source: attr_string(e, "source").filter(is_meaningful),
        label: attr_string(e, "label").filter(|s| !s.is_empty()),
        main_active_skill: attr_u32(e, "mainActiveSkill"),
        gems: Vec::new(),
    }
}

fn read_gem(e: &BytesStart<'_>) -> Gem {
    let name_spec = attr_string(e, "nameSpec");
    let skill_id = attr_string(e, "skillId");
    let gem_id = attr_string(e, "gemId");
    let is_support =
        derive_is_support(gem_id.as_deref(), skill_id.as_deref(), name_spec.as_deref());
    Gem {
        name_spec,
        skill_id,
        gem_id,
        level: attr_u32(e, "level"),
        quality: attr_u32(e, "quality"),
        enabled: attr_bool(e, "enabled").unwrap_or(true),
        is_support,
    }
}

/// Support gems are identifiable from any of the three ids. `gemId` is the most reliable.
fn derive_is_support(
    gem_id: Option<&str>,
    skill_id: Option<&str>,
    name_spec: Option<&str>,
) -> bool {
    gem_id.is_some_and(|g| g.contains("SupportGem"))
        || skill_id.is_some_and(|s| s.starts_with("Support"))
        || name_spec.is_some_and(|n| n.trim_end().ends_with("Support"))
}

/// PoB writes "None"/"Nothing" for an unset ascendancy/source. Treat those (and empties) as absent.
fn is_meaningful(s: &String) -> bool {
    !s.is_empty() && s != "None" && s != "Nothing"
}

fn parse_nodes(s: String) -> Vec<u32> {
    s.split(',')
        .filter_map(|t| {
            let t = t.trim();
            if t.is_empty() {
                None
            } else {
                t.parse::<u32>().ok()
            }
        })
        .collect()
}

/// The local part of a (possibly namespaced) element name, as an owned `String`. PoB uses no
/// namespaces, but taking the local name keeps this robust and avoids borrowing the event buffer.
fn local_name(qname: &[u8]) -> String {
    let local = match qname.iter().rposition(|&b| b == b':') {
        Some(i) => &qname[i + 1..],
        None => qname,
    };
    String::from_utf8_lossy(local).into_owned()
}

fn attr_string(e: &BytesStart<'_>, name: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.local_name().as_ref() == name.as_bytes() {
            // Normalize as XML 1.0 (resolves entities like &amp; in spec/skill titles).
            return attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|v| v.into_owned());
        }
    }
    None
}

fn attr_u32(e: &BytesStart<'_>, name: &str) -> Option<u32> {
    attr_string(e, name).and_then(|s| s.trim().parse().ok())
}

fn attr_u8(e: &BytesStart<'_>, name: &str) -> Option<u8> {
    attr_string(e, name).and_then(|s| s.trim().parse().ok())
}

fn attr_usize(e: &BytesStart<'_>, name: &str) -> Option<usize> {
    attr_string(e, name).and_then(|s| s.trim().parse().ok())
}

/// PoB booleans are "true"/"false" (occasionally "nil"). Absent -> `None` so callers pick a default.
fn attr_bool(e: &BytesStart<'_>, name: &str) -> Option<bool> {
    attr_string(e, name).map(|s| matches!(s.trim(), "true" | "True" | "1"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" className="Witch" ascendClassName="Necromancer" mainSocketGroup="1" targetVersion="3_25">
    <PlayerStat stat="Life" value="4200"/>
  </Build>
  <Tree activeSpec="2">
    <Spec title="Leaguestart" treeVersion="3_25" classId="3" nodes="1,2,3"/>
    <Spec title="Level 68 (Endgame)" treeVersion="3_25" classId="3" ascendClassId="1" nodes="10, 20 ,30,">
      <URL>https://www.pathofexile.com/passive-skill-tree/xyz</URL>
    </Spec>
  </Tree>
  <Skills activeSkillSet="1">
    <SkillSet id="1" title="Main">
      <Skill enabled="true" slot="Body Armour" mainActiveSkill="1">
        <Gem nameSpec="Raise Zombie" skillId="RaiseZombie" gemId="Metadata/Items/Gems/SkillGemRaiseZombie" level="20" quality="0"/>
        <Gem nameSpec="Spell Echo Support" skillId="SupportSpellEcho" gemId="Metadata/Items/Gems/SupportGemSpellEcho" level="20" quality="20"/>
      </Skill>
    </SkillSet>
  </Skills>
</PathOfBuilding>"#;

    fn parse(s: &str) -> PobBuild {
        parse_xml(s.as_bytes()).expect("parse")
    }

    #[test]
    fn reads_build_metadata() {
        let b = parse(SAMPLE);
        assert_eq!(b.level, Some(90));
        assert_eq!(b.class_name.as_deref(), Some("Witch"));
        assert_eq!(b.ascend_class_name.as_deref(), Some("Necromancer"));
        assert_eq!(b.main_socket_group, Some(1));
        assert_eq!(b.target_version.as_deref(), Some("3_25"));
    }

    #[test]
    fn reads_specs_and_selects_active() {
        let b = parse(SAMPLE);
        assert_eq!(b.specs.len(), 2);
        assert_eq!(b.active_spec, 2);
        let sel = b.selected_spec().expect("selected");
        assert_eq!(sel.title.as_deref(), Some("Level 68 (Endgame)"));
        assert_eq!(sel.ascend_class_id, Some(1));
        // Trailing/interior spaces and a trailing comma in `nodes` are tolerated.
        assert_eq!(sel.nodes, vec![10, 20, 30]);
        assert_eq!(b.specs[0].nodes, vec![1, 2, 3]);
    }

    #[test]
    fn reads_skillset_gems_and_flags_support() {
        let b = parse(SAMPLE);
        let group = b.main_group().expect("main group");
        assert_eq!(group.slot.as_deref(), Some("Body Armour"));
        assert_eq!(group.gems.len(), 2);
        let active = group.first_active_gem().expect("active gem");
        assert_eq!(active.name_spec.as_deref(), Some("Raise Zombie"));
        assert!(!active.is_support);
        assert!(
            group.gems[1].is_support,
            "Spell Echo Support is a support gem"
        );
    }

    #[test]
    fn handles_legacy_skills_without_a_skillset() {
        let xml = r#"<PathOfBuilding>
            <Build className="Marauder"/>
            <Skills>
                <Skill slot="Weapon 1"><Gem nameSpec="Cleave" gemId="Metadata/Items/Gems/SkillGemCleave"/></Skill>
            </Skills>
        </PathOfBuilding>"#;
        let b = parse(xml);
        assert_eq!(
            b.skill_sets.len(),
            1,
            "legacy skills fold into one synthetic set"
        );
        assert_eq!(b.skill_sets[0].id, None);
        let group = b.main_group().expect("group");
        assert_eq!(
            group.first_active_gem().unwrap().name_spec.as_deref(),
            Some("Cleave")
        );
    }

    #[test]
    fn tolerates_missing_attributes() {
        let xml = r#"<PathOfBuilding>
            <Build/>
            <Tree><Spec/></Tree>
            <Skills><SkillSet id="1"><Skill><Gem/></Skill></SkillSet></Skills>
        </PathOfBuilding>"#;
        let b = parse(xml);
        assert_eq!(b.level, None);
        assert_eq!(b.class_name, None);
        assert_eq!(
            b.active_spec, 1,
            "defaults to spec 1 when activeSpec is absent"
        );
        assert_eq!(b.specs.len(), 1);
        assert!(b.specs[0].title.is_none());
        assert!(b.specs[0].nodes.is_empty());
        let gem = b.main_group().unwrap().gems.first().unwrap();
        assert!(
            gem.enabled,
            "a gem with no enabled attribute defaults to enabled"
        );
    }

    #[test]
    fn unset_ascendancy_is_none() {
        let xml =
            r#"<PathOfBuilding><Build className="Witch" ascendClassName="None"/></PathOfBuilding>"#;
        assert_eq!(parse(xml).ascend_class_name, None);
    }

    #[test]
    fn rejects_non_pob_root() {
        let err = parse_xml(b"<html><body>nope</body></html>").unwrap_err();
        assert!(matches!(err, PobError::NotPob { found } if found == "html"));
    }
}
