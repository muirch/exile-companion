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
    fn remembers_the_character() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_link(dir.path()).unwrap(), None);
        let link = CharacterLink::new("Exile#1234", "RealOne", "pc");
        write_link(dir.path(), &link).unwrap();
        assert_eq!(read_link(dir.path()).unwrap(), Some(link));
    }
}
