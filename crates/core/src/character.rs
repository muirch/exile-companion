//! The player's own character on pathofexile.com, remembered so a refresh needs only a click.
//! Read through the same public character API Path of Building's Import tab uses.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::build_history::{now, write_atomic};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterLink {
    /// As the profile shows it, with the #1234 the site adds.
    pub account: String,
    pub character: String,
    /// "pc", "xbox" or "sony".
    pub realm: String,
    /// RFC 3339, UTC.
    pub imported: String,
}

impl CharacterLink {
    pub fn new(account: &str, character: &str, realm: &str) -> Self {
        Self {
            account: account.to_string(),
            character: character.to_string(),
            realm: realm.to_string(),
            imported: now(),
        }
    }
}

/// character.json next to the app's build.xml.
pub fn link_file(build_dir: &Path) -> PathBuf {
    build_dir.join("character.json")
}

pub fn read_link(build_dir: &Path) -> std::io::Result<Option<CharacterLink>> {
    match std::fs::read(link_file(build_dir)) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(std::io::Error::other),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn write_link(build_dir: &Path, link: &CharacterLink) -> std::io::Result<()> {
    std::fs::create_dir_all(build_dir)?;
    write_atomic(&link_file(build_dir), &serde_json::to_vec_pretty(link)?)
}

/// What a refresh changed in the build: level, passive points and the items that weren't there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Refreshed {
    /// None on the first import, when there was nothing to compare with.
    pub level_before: Option<u32>,
    pub level: Option<u32>,
    pub points_before: usize,
    pub points: usize,
    /// "Name, Base" for rares and uniques. Empty on the first import.
    pub new_items: Vec<String>,
}

/// Compares the build XML from before a refresh with the one after. `before` is empty on the
/// first import.
pub fn refreshed(before: &[u8], after: &[u8]) -> Refreshed {
    let read = |xml: &[u8]| {
        (!xml.is_empty())
            .then(|| exile_pob::parse_xml(xml).ok())
            .flatten()
    };
    let points = |b: &Option<exile_pob::PobBuild>| {
        b.as_ref()
            .and_then(|b| b.selected_spec())
            .map_or(0, |s| s.nodes.len())
    };
    let (old, new) = (read(before), read(after));
    let mut new_items = Vec::new();
    if old.is_some() {
        let mut had = exile_pob::item_names(before).unwrap_or_default();
        for name in exile_pob::item_names(after).unwrap_or_default() {
            match had.iter().position(|h| *h == name) {
                Some(i) => {
                    had.remove(i);
                }
                None => new_items.push(name),
            }
        }
    }
    Refreshed {
        level_before: old.as_ref().and_then(|b| b.level),
        level: new.as_ref().and_then(|b| b.level),
        points_before: points(&old),
        points: points(&new),
        new_items,
    }
}

/// The way PoB reads what the player typed: no spaces, and "Name-1234" taken as "Name#1234".
pub fn account_name(input: &str) -> String {
    let name: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    match name.rfind(['#', '-']) {
        Some(at) => format!("{}#{}", &name[..at], &name[at + 1..]),
        None => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_account_names_the_way_pob_does() {
        assert_eq!(account_name(" Exile#1234 "), "Exile#1234");
        assert_eq!(account_name("Exile-1234"), "Exile#1234");
        assert_eq!(account_name("Some Exile#77"), "SomeExile#77");
        assert_eq!(account_name("Exile"), "Exile");
    }

    #[test]
    fn says_what_a_refresh_changed() {
        let build = |level: u32, nodes: &str, items: &str| {
            format!(
                "<PathOfBuilding><Build level=\"{level}\"/><Tree activeSpec=\"1\"><Spec nodes=\"{nodes}\"/></Tree><Items>{items}</Items></PathOfBuilding>"
            )
        };
        let item = |id: u32, name: &str| {
            format!("<Item id=\"{id}\">Rarity: RARE\n{name}\nJudgement Staff\nImplicits: 0</Item>")
        };
        let before = build(14, "1,2,3", &item(1, "Old Staff"));
        let after = build(
            22,
            "1,2,3,4,5",
            &format!("{}{}", item(1, "Old Staff"), item(2, "New Staff")),
        );
        let r = refreshed(before.as_bytes(), after.as_bytes());
        assert_eq!((r.level_before, r.level), (Some(14), Some(22)));
        assert_eq!((r.points_before, r.points), (3, 5));
        assert_eq!(r.new_items, vec!["New Staff, Judgement Staff"]);

        let first = refreshed(b"", after.as_bytes());
        assert_eq!(first.level_before, None);
        assert!(first.new_items.is_empty());
    }

    #[test]
    fn remembers_the_character() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_link(dir.path()).unwrap(), None);
        let link = CharacterLink::new("Exile#1234", "RealOne", "pc");
        write_link(dir.path(), &link).unwrap();
        assert_eq!(read_link(dir.path()).unwrap(), Some(link));
    }
}
