//! An item's text in PoB's own format (`Item:BuildRaw` in PoB): the rarity, the name lines, a
//! header of `Key: value` lines, `Implicits: N`, then the mod lines with `{tag}` prefixes.

/// What a mod line is, for the colour the game draws it in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModKind {
    Enchant,
    Implicit,
    Explicit,
    Crafted,
    Fractured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLine {
    /// Without PoB's tags. A roll range stays as PoB has it, "(25-27)% increased ...".
    pub text: String,
    pub kind: ModKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemText {
    /// As PoB writes it: "NORMAL", "MAGIC", "RARE", "UNIQUE", "RELIC".
    pub rarity: String,
    pub name: String,
    /// The base under a rare's or unique's name. A normal or magic item has it in its name.
    pub base: Option<String>,
    pub quality: Option<u32>,
    /// "R-R-R-R B B", linked sockets joined by "-".
    pub sockets: Option<String>,
    pub level_req: Option<u32>,
    /// A jewel's radius, "Small", "Medium", "Large", "Variable".
    pub radius: Option<String>,
    pub corrupted: bool,
    pub mods: Vec<ModLine>,
    /// A cluster jewel's small passive, as PoB names it: "affliction_minion_damage".
    pub cluster_skill: Option<String>,
    pub cluster_node_count: Option<u32>,
}

impl ItemText {
    pub fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        let rarity = lines.next()?.strip_prefix("Rarity:")?.trim().to_uppercase();
        let name = lines.next()?.to_string();
        let base = matches!(rarity.as_str(), "RARE" | "UNIQUE" | "RELIC")
            .then(|| lines.next().map(String::from))
            .flatten();
        let mut item = Self {
            rarity,
            name,
            base,
            ..Self::default()
        };
        let mut variant: Option<u32> = None;
        let mut implicits = None;
        for line in lines.by_ref() {
            if let Some(n) = line.strip_prefix("Implicits:") {
                implicits = n.trim().parse::<usize>().ok();
                break;
            }
            let (key, value) = line.split_once(':').unwrap_or((line, ""));
            let value = value.trim();
            match key {
                "Quality" => item.quality = value.parse().ok(),
                "Sockets" => item.sockets = Some(value.to_string()),
                "LevelReq" => item.level_req = value.parse().ok(),
                "Radius" => item.radius = Some(value.to_string()),
                "Selected Variant" => variant = value.parse().ok(),
                "Cluster Jewel Skill" => item.cluster_skill = Some(value.to_string()),
                "Cluster Jewel Node Count" => item.cluster_node_count = value.parse().ok(),
                _ => {}
            }
        }
        let implicits = implicits?;
        let mut n = 0;
        for line in lines {
            if matches!(line, "Corrupted" | "Mirrored" | "Split" | "Fractured Item")
                || line.starts_with("Foil Unique")
            {
                item.corrupted |= line == "Corrupted";
                continue;
            }
            let implicit = n < implicits;
            n += 1;
            if let Some(m) = mod_line(line, implicit, variant) {
                item.mods.push(m);
            }
        }
        Some(item)
    }

    /// The notables an anoint allocates, "Death Attunement" for "Allocates Death Attunement".
    pub fn allocates(&self) -> impl Iterator<Item = &str> {
        self.mods
            .iter()
            .filter_map(|m| m.text.strip_prefix("Allocates "))
    }
}

/// One mod line without its tags, or None for a line the selected variant doesn't have.
fn mod_line(line: &str, implicit: bool, variant: Option<u32>) -> Option<ModLine> {
    let mut rest = line;
    let mut kind = if implicit {
        ModKind::Implicit
    } else {
        ModKind::Explicit
    };
    while let Some(tag_end) = rest.strip_prefix('{').and_then(|r| r.find('}')) {
        let tag = &rest[1..=tag_end];
        rest = &rest[tag_end + 2..];
        let (name, value) = tag.split_once(':').unwrap_or((tag, ""));
        match name {
            "crafted" => kind = ModKind::Crafted,
            "fractured" => kind = ModKind::Fractured,
            "enchant" => kind = ModKind::Enchant,
            "variant" => {
                let has = |v: u32| value.split(',').any(|id| id.trim().parse() == Ok(v));
                if variant.is_some_and(|v| !has(v)) {
                    return None;
                }
            }
            "disabled" => return None,
            _ => {}
        }
    }
    Some(ModLine {
        text: rest.to_string(),
        kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAFF: &str = "Rarity: RARE
New Item
Judgement Staff
Crafted: true
Prefix: {range:0.289}LocalAddedFireDamageTwoHand8_
Prefix: None
Suffix: {range:0.69}LocalIncreasedAttackSpeed4
Quality: 20
Sockets: B-B-B-B-B-B
LevelReq: 68
Implicits: 1
{tags:block}+25% Chance to Block Attack Damage
{tags:physical_damage,damage,physical,attack}{prefix}130% increased Physical Damage
{tags:attack,critical}{crafted}{range:0.5}(25-27)% increased Critical Strike Chance
Corrupted";

    #[test]
    fn reads_a_rare_with_its_mods() {
        let item = ItemText::parse(STAFF).unwrap();
        assert_eq!(item.rarity, "RARE");
        assert_eq!(item.name, "New Item");
        assert_eq!(item.base.as_deref(), Some("Judgement Staff"));
        assert_eq!(item.quality, Some(20));
        assert_eq!(item.sockets.as_deref(), Some("B-B-B-B-B-B"));
        assert_eq!(item.level_req, Some(68));
        assert!(item.corrupted);
        let mods: Vec<(&str, ModKind)> = item
            .mods
            .iter()
            .map(|m| (m.text.as_str(), m.kind))
            .collect();
        assert_eq!(
            mods,
            vec![
                ("+25% Chance to Block Attack Damage", ModKind::Implicit),
                ("130% increased Physical Damage", ModKind::Explicit),
                (
                    "(25-27)% increased Critical Strike Chance",
                    ModKind::Crafted
                ),
            ]
        );
    }

    #[test]
    fn keeps_only_the_selected_variant_and_finds_the_anoint() {
        let text = "Rarity: UNIQUE
Bonemeld
Marble Amulet
Variant: Pre 3.16.0
Variant: Current
Selected Variant: 2
LevelReq: 74
Implicits: 2
{crafted}Allocates Death Attunement
{tags:resource}{range:0.5}Regenerate (1.2-1.6)% of Life per second
{variant:1}+1 to Level of all Minion Skill Gems
{variant:2}{range:0.5}+(1-2) to Level of all Minion Skill Gems";
        let item = ItemText::parse(text).unwrap();
        let mods: Vec<&str> = item.mods.iter().map(|m| m.text.as_str()).collect();
        assert_eq!(
            mods,
            vec![
                "Allocates Death Attunement",
                "Regenerate (1.2-1.6)% of Life per second",
                "+(1-2) to Level of all Minion Skill Gems",
            ]
        );
        assert_eq!(item.mods[0].kind, ModKind::Crafted);
        assert_eq!(
            item.allocates().collect::<Vec<_>>(),
            vec!["Death Attunement"]
        );
    }

    #[test]
    fn reads_a_jewel_radius_and_a_magic_name() {
        let jewel = ItemText::parse(
            "Rarity: UNIQUE\nBrutal Restraint\nTimeless Jewel\nRadius: Large\nLimited to: 1\nImplicits: 0\nHistoric",
        )
        .unwrap();
        assert_eq!(jewel.radius.as_deref(), Some("Large"));
        assert_eq!(jewel.mods[0].kind, ModKind::Explicit);
        let flask =
            ItemText::parse("Rarity: MAGIC\nSeething Divine Life Flask\nImplicits: 0").unwrap();
        assert_eq!(flask.name, "Seething Divine Life Flask");
        assert_eq!(flask.base, None);
        assert!(ItemText::parse("Rarity: RARE\nNo implicits line").is_none());
    }
}
