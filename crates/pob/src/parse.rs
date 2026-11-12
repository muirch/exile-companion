//! Parse Path of Building XML into a [`PobBuild`].
//!
//! A streaming quick-xml walk. Children are matched BY NAME (document order is not guaranteed) and
//! every attribute is optional. We only read the elements we care about (`Build`, `Tree`/`Spec`
//! with its `Sockets` and `Overrides`, `Skills`/`SkillSet`/`Skill`/`Gem`, `Items`/`Item`/`ItemSet`,
//! `Notes`) and ignore the rest (`Calcs`, `PlayerStat`, `URL`, ...). Both
//! `<Skill>`-directly-under-`<Skills>` (legacy) and the newer `<SkillSet>` grouping are handled.

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use crate::error::PobError;
use crate::model::{Gem, Item, ItemSet, NodeOverride, PobBuild, SkillGroup, SkillSet, TreeSpec};

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
            Event::Text(t) if p.text_target().is_some() => {
                let text = t.xml10_content().map_err(quick_xml::Error::from)?;
                p.text(&text);
            }
            Event::CData(t) if p.text_target().is_some() => {
                let text = t.decode().map_err(quick_xml::Error::from)?;
                p.text(&text);
            }
            Event::GeneralRef(r) if p.text_target().is_some() => {
                let c = match r.resolve_char_ref()? {
                    Some(c) => Some(c),
                    None => match r.as_ref() {
                        b"amp" => Some('&'),
                        b"lt" => Some('<'),
                        b"gt" => Some('>'),
                        b"quot" => Some('"'),
                        b"apos" => Some('\''),
                        _ => None,
                    },
                };
                if let Some(c) = c {
                    p.text(c.encode_utf8(&mut [0; 4]));
                }
            }
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
    /// Text of `<Notes>` while inside it.
    notes: Option<String>,
    cur_item: Option<Item>,
    cur_item_set: Option<ItemSet>,
    cur_override: Option<NodeOverride>,
    /// The text of a tattoo override, split into stats when it closes.
    override_text: String,
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
            "Notes" if !empty => self.notes = Some(String::new()),
            "Items" => self.build.active_item_set = attr_u32(e, "activeItemSet"),
            "Item" if !empty => {
                self.cur_item = attr_u32(e, "id").map(|id| Item {
                    id,
                    text: String::new(),
                });
            }
            "ItemSet" => {
                let set = ItemSet {
                    id: attr_u32(e, "id"),
                    title: attr_string(e, "title"),
                    slots: Vec::new(),
                };
                if empty {
                    self.build.item_sets.push(set);
                } else {
                    self.cur_item_set = Some(set);
                }
            }
            "Slot" => {
                if let (Some(set), Some(name), Some(id)) = (
                    self.cur_item_set.as_mut(),
                    attr_string(e, "name"),
                    attr_u32(e, "itemId").filter(|id| *id != 0),
                ) {
                    set.slots.push((name, id));
                }
            }
            "Socket" => {
                if let (Some(spec), Some(node), Some(item)) = (
                    self.cur_spec.as_mut(),
                    attr_u32(e, "nodeId"),
                    attr_u32(e, "itemId").filter(|id| *id != 0),
                ) {
                    spec.sockets.push((node, item));
                }
            }
            "Override" if self.cur_spec.is_some() => {
                let tattoo = NodeOverride {
                    node: attr_u32(e, "nodeId").unwrap_or_default(),
                    name: attr_string(e, "dn").unwrap_or_default(),
                    stats: Vec::new(),
                };
                self.override_text.clear();
                if empty {
                    self.push_override(tattoo);
                } else {
                    self.cur_override = Some(tattoo);
                }
            }
            _ => {}
        }
    }

    /// Where text goes right now, if anywhere.
    fn text_target(&mut self) -> Option<&mut String> {
        if let Some(notes) = self.notes.as_mut() {
            return Some(notes);
        }
        if let Some(item) = self.cur_item.as_mut() {
            return Some(&mut item.text);
        }
        self.cur_override
            .is_some()
            .then_some(&mut self.override_text)
    }

    fn text(&mut self, text: &str) {
        if let Some(target) = self.text_target() {
            target.push_str(text);
        }
    }

    fn push_override(&mut self, mut tattoo: NodeOverride) {
        tattoo.stats = self
            .override_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();
        if let Some(spec) = self.cur_spec.as_mut() {
            spec.overrides.push(tattoo);
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
            "Item" => {
                if let Some(mut item) = self.cur_item.take() {
                    item.text = item.text.replace('\r', "").trim().to_string();
                    self.build.items.push(item);
                }
            }
            "ItemSet" => {
                if let Some(set) = self.cur_item_set.take() {
                    self.build.item_sets.push(set);
                }
            }
            "Override" => {
                if let Some(tattoo) = self.cur_override.take() {
                    self.push_override(tattoo);
                }
            }
            "Notes" => {
                if let Some(notes) = self.notes.take() {
                    let notes = notes.replace('\r', "");
                    let notes = notes.trim();
                    self.build.notes =
                        (!plain_notes(notes).trim().is_empty()).then(|| notes.to_string());
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

/// The notes without PoB's colour codes (`^7`, `^xRRGGBB`), for whoever reads only the text.
pub fn plain_notes(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '^' if chars.peek().is_some_and(char::is_ascii_digit) => {
                chars.next();
            }
            '^' if chars.peek() == Some(&'x') => {
                let rest: String = chars.clone().skip(1).take(6).collect();
                if rest.len() == 6 && rest.chars().all(|c| c.is_ascii_hexdigit()) {
                    chars.nth(6);
                } else {
                    out.push(c);
                }
            }
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

fn read_spec(e: &BytesStart<'_>) -> TreeSpec {
    TreeSpec {
        title: attr_string(e, "title"),
        tree_version: attr_string(e, "treeVersion"),
        class_id: attr_u8(e, "classId"),
        ascend_class_id: attr_u8(e, "ascendClassId"),
        nodes: attr_string(e, "nodes").map(parse_nodes).unwrap_or_default(),
        mastery_effects: attr_string(e, "masteryEffects")
            .map(|s| parse_mastery_effects(&s))
            .unwrap_or_default(),
        sockets: Vec::new(),
        overrides: Vec::new(),
    }
}

/// PoB writes them as "{node,effect},{node,effect}".
fn parse_mastery_effects(s: &str) -> Vec<(u32, u32)> {
    s.split('}')
        .filter_map(|pair| {
            let (node, effect) = pair.trim_start_matches([',', '{', ' ']).split_once(',')?;
            Some((node.trim().parse().ok()?, effect.trim().parse().ok()?))
        })
        .collect()
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
    <Spec title="Level 68 (Endgame)" treeVersion="3_25" classId="3" ascendClassId="1" nodes="10, 20 ,30," masteryEffects="{38595,9755},{ 4119, 48385 }">
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
    fn keeps_the_notes_colour_codes_and_drops_them_on_request() {
        let b = parse(
            "<PathOfBuilding><Build level=\"1\"/><Notes>\r\n  ^xE05030Act 1^7 -&#10;Help Alira &amp; skip ^xZZ<![CDATA[ <ok>]]>\r\n</Notes></PathOfBuilding>",
        );
        let notes = b.notes.unwrap();
        assert_eq!(notes, "^xE05030Act 1^7 -\nHelp Alira & skip ^xZZ <ok>");
        assert_eq!(plain_notes(&notes), "Act 1 -\nHelp Alira & skip ^xZZ <ok>");
        let only_codes = parse("<PathOfBuilding><Notes>^7 ^xFFFFFF</Notes></PathOfBuilding>");
        assert_eq!(only_codes.notes, None);
    }

    #[test]
    fn empty_notes_are_none() {
        assert_eq!(
            parse("<PathOfBuilding><Notes>\n  </Notes></PathOfBuilding>").notes,
            None
        );
        assert_eq!(
            parse("<PathOfBuilding><Notes/></PathOfBuilding>").notes,
            None
        );
        assert_eq!(parse(SAMPLE).notes, None);
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
        assert_eq!(sel.mastery_effects, vec![(38595, 9755), (4119, 48385)]);
        assert!(b.specs[0].mastery_effects.is_empty());
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
    fn reads_items_gear_sets_jewels_and_tattoos() {
        let xml = r#"<PathOfBuilding>
<Tree activeSpec="1"><Spec title="Midgame" nodes="1,2">
  <Sockets><Socket itemId="7" nodeId="26725"/><Socket itemId="0" nodeId="7960"/></Sockets>
  <Overrides>
    <Override dn="Tattoo of the Kitava Shaman" nodeId="60472">
      Attacks have 5% chance to cause Bleeding
    </Override>
  </Overrides>
</Spec></Tree>
<Items activeItemSet="21">
  <Item id="7">
    Rarity: RARE
New Item
Ghastly Eye Jewel
Implicits: 0
Minions deal 18% increased Damage if you&apos;ve used a Minion Skill Recently
    <ModRange id="1" range="0.5"/>
  </Item>
  <ItemSet title="Midgame" id="21">
    <Slot itemId="7" name="Weapon 1"/>
    <Slot itemId="0" name="Helmet"/>
  </ItemSet>
</Items>
</PathOfBuilding>"#;
        let b = parse(xml);
        let spec = &b.specs[0];
        assert_eq!(spec.sockets, vec![(26725, 7)]);
        assert_eq!(spec.overrides.len(), 1);
        assert_eq!(spec.overrides[0].node, 60472);
        assert_eq!(spec.overrides[0].name, "Tattoo of the Kitava Shaman");
        assert_eq!(
            spec.overrides[0].stats,
            vec!["Attacks have 5% chance to cause Bleeding"]
        );
        assert_eq!(b.active_item_set, Some(21));
        assert_eq!(b.item_sets[0].slots, vec![("Weapon 1".to_string(), 7)]);
        let item = b.item(7).unwrap();
        assert!(item.text.starts_with("Rarity: RARE\nNew Item"));
        assert!(item
            .text
            .ends_with("if you've used a Minion Skill Recently"));
        assert_eq!(b.item_set_for(0).and_then(|s| s.id), Some(21));
    }

    #[test]
    fn rejects_non_pob_root() {
        let err = parse_xml(b"<html><body>nope</body></html>").unwrap_err();
        assert!(matches!(err, PobError::NotPob { found } if found == "html"));
    }
}
