//! Language-specific message templates, isolated so non-English clients become a later data add-on
//! rather than a parser rewrite.
//!
//! `Client.txt` localizes both the "You have entered ..." template *and* zone names. Phase 1 ships the
//! English pack only. The seam exists so [`LocalePack::EN`] can be joined by other packs (eventually
//! sourced from the data pack) without touching the parser.

/// The set of localized substrings the parser matches for system events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalePack {
    /// Prefix of the zone-entry line body, e.g. `"You have entered "`.
    pub entered_prefix: &'static str,
    /// Suffix of the zone-entry line body, e.g. `"."`.
    pub entered_suffix: &'static str,
    /// Suffix of the "slain" death line, e.g. `" has been slain."`.
    pub slain_suffix: &'static str,
    /// Suffix of the "suicide" death line, e.g. `" has committed suicide."`.
    pub suicide_suffix: &'static str,
}

impl LocalePack {
    /// English client templates.
    pub const EN: LocalePack = LocalePack {
        entered_prefix: "You have entered ",
        entered_suffix: ".",
        slain_suffix: " has been slain.",
        suicide_suffix: " has committed suicide.",
    };
}

impl Default for LocalePack {
    fn default() -> Self {
        LocalePack::EN
    }
}
