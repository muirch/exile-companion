//! The guide the player follows, kept between conversations with the assistant. The guide's build
//! is stored as PoB XML, so a later conversation needs neither the link nor the network.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::build_history::{now, write_atomic};

/// A guide stage by the ids in its PoB: tree, item set and skill set. None is the guide's own choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stage {
    pub tree: Option<u32>,
    pub item_set: Option<u32>,
    pub skill_set: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Followed {
    /// The link or page the guide came from, when it came from one.
    pub source: Option<String>,
    /// RFC 3339, UTC.
    pub since: String,
    pub stage: Stage,
}

pub struct GuideStore {
    dir: PathBuf,
}

impl GuideStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn follow(
        &self,
        xml: &str,
        source: Option<&str>,
        stage: Stage,
    ) -> std::io::Result<Followed> {
        let followed = Followed {
            source: source.map(str::to_string),
            since: now(),
            stage,
        };
        std::fs::create_dir_all(&self.dir)?;
        write_atomic(&self.dir.join("guide.xml"), xml.as_bytes())?;
        self.write(&followed)?;
        Ok(followed)
    }

    /// The guide and its build XML, or None when the player follows none.
    pub fn get(&self) -> std::io::Result<Option<(Followed, String)>> {
        let meta = match std::fs::read(self.dir.join("guide.json")) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let followed = serde_json::from_slice(&meta).map_err(std::io::Error::other)?;
        let xml = std::fs::read_to_string(self.dir.join("guide.xml"))?;
        Ok(Some((followed, xml)))
    }

    pub fn set_stage(&self, stage: Stage) -> std::io::Result<Option<Followed>> {
        let Some((mut followed, _)) = self.get()? else {
            return Ok(None);
        };
        followed.stage = stage;
        self.write(&followed)?;
        Ok(Some(followed))
    }

    pub fn forget(&self) -> std::io::Result<()> {
        for name in ["guide.json", "guide.xml"] {
            match std::fs::remove_file(self.dir.join(name)) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    // Written after guide.xml, so a guide.xml without it counts as no guide.
    fn write(&self, followed: &Followed) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(followed).map_err(std::io::Error::other)?;
        write_atomic(&self.dir.join("guide.json"), &json)
    }
}

/// Whether a build carries exactly the guide's items, as it does after the guide became the
/// player's build. Its gear then says nothing about what the character wears.
pub fn has_guide_gear(build_xml: &[u8], guide_xml: &[u8]) -> bool {
    match (
        exile_pob::item_names(build_xml),
        exile_pob::item_names(guide_xml),
    ) {
        (Ok(build), Ok(guide)) => !build.is_empty() && build == guide,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_a_build_with_the_guides_items() {
        let items = |names: &[&str]| {
            let items: String = names
                .iter()
                .map(|n| format!("<Item>Rarity: MAGIC\n{n}\n</Item>"))
                .collect();
            format!("<PathOfBuilding><Items>{items}</Items></PathOfBuilding>")
        };
        let guide = items(&["Iron Ring", "Rustic Sash"]);
        assert!(has_guide_gear(
            items(&["Rustic Sash", "Iron Ring"]).as_bytes(),
            guide.as_bytes()
        ));
        assert!(!has_guide_gear(
            items(&["Iron Ring"]).as_bytes(),
            guide.as_bytes()
        ));
        assert!(!has_guide_gear(
            items(&[]).as_bytes(),
            items(&[]).as_bytes()
        ));
        assert!(!has_guide_gear(b"not xml <", guide.as_bytes()));
    }

    #[test]
    fn keeps_the_guide_and_its_stage() {
        let dir = tempfile::tempdir().unwrap();
        let store = GuideStore::new(dir.path());
        assert_eq!(store.get().unwrap(), None);

        let stage = Stage {
            tree: Some(2),
            ..Stage::default()
        };
        store
            .follow("<PathOfBuilding/>", Some("https://pobb.in/abc"), stage)
            .unwrap();
        let (followed, xml) = store.get().unwrap().unwrap();
        assert_eq!(xml, "<PathOfBuilding/>");
        assert_eq!(followed.source.as_deref(), Some("https://pobb.in/abc"));
        assert_eq!(followed.stage.tree, Some(2));

        let later = Stage {
            tree: Some(3),
            skill_set: Some(2),
            item_set: None,
        };
        store.set_stage(later).unwrap();
        assert_eq!(store.get().unwrap().unwrap().0.stage, later);

        store.forget().unwrap();
        assert_eq!(store.get().unwrap(), None);
        assert_eq!(store.set_stage(later).unwrap(), None);
    }
}
