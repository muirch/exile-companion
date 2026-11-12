//! A build's item as the overlay draws it: the game's tooltip, from PoB's item text.

use exile_pob::{ItemText, ModKind};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
    /// "normal", "magic", "rare", "unique", "relic".
    pub rarity: String,
    pub name: String,
    pub base: Option<String>,
    pub quality: Option<u32>,
    pub sockets: Option<String>,
    pub level_req: Option<u32>,
    pub radius: Option<String>,
    pub corrupted: bool,
    pub mods: Vec<ModView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModView {
    pub text: String,
    pub kind: ModTone,
}

/// The colour group of a mod line in the game's tooltip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModTone {
    Enchant,
    Implicit,
    Explicit,
    Crafted,
    Fractured,
}

impl ItemView {
    pub fn parse(text: &str) -> Option<Self> {
        ItemText::parse(text).map(|t| Self::from(&t))
    }
}

impl From<&ItemText> for ItemView {
    fn from(t: &ItemText) -> Self {
        Self {
            rarity: t.rarity.to_lowercase(),
            name: t.name.clone(),
            base: t.base.clone(),
            quality: t.quality.filter(|q| *q > 0),
            sockets: t.sockets.clone(),
            level_req: t.level_req.filter(|l| *l > 0),
            radius: t.radius.clone(),
            corrupted: t.corrupted,
            mods: t
                .mods
                .iter()
                .map(|m| ModView {
                    text: m.text.clone(),
                    kind: match m.kind {
                        ModKind::Enchant => ModTone::Enchant,
                        ModKind::Implicit => ModTone::Implicit,
                        ModKind::Explicit => ModTone::Explicit,
                        ModKind::Crafted => ModTone::Crafted,
                        ModKind::Fractured => ModTone::Fractured,
                    },
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_what_the_tooltip_shows() {
        let v = ItemView::parse(
            "Rarity: RARE\nNew Item\nFoul Staff\nQuality: 0\nLevelReq: 55\nImplicits: 1\n+22% Chance to Block\n{crafted}(80-99)% increased Physical Damage",
        )
        .unwrap();
        assert_eq!(v.rarity, "rare");
        assert_eq!(v.base.as_deref(), Some("Foul Staff"));
        assert_eq!(v.quality, None);
        assert_eq!(v.level_req, Some(55));
        assert_eq!(v.mods[0].kind, ModTone::Implicit);
        assert_eq!(v.mods[1].kind, ModTone::Crafted);
    }
}
