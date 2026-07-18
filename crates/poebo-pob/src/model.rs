//! The parsed shape of a Path of Building build.
//!
//! Everything is optional. PoB has no formal schema and the community fork has added attributes over
//! the years (`secondaryAscendClassId`, `masteryEffects`, `<SkillSet>`, `qualityId`, ...), so the
//! parser treats every attribute as best-effort and this model mirrors that: absent fields are
//! `None`, not errors.

/// A whole imported build: build-level metadata, the passive-tree specs, and the skill/gem groups.
///
/// A build carries no per-spec level or act (the KEY FACT): leveling stage lives only in each spec's
/// free-text [`TreeSpec::title`]. The tree resolver applies title heuristics on top of this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PobBuild {
    /// The build's overall character level (the endgame/target level, not the current one).
    pub level: Option<u32>,
    /// Base class name, e.g. "Witch".
    pub class_name: Option<String>,
    /// Ascendancy name, e.g. "Necromancer"; `None` when unset (PoB writes "None").
    pub ascend_class_name: Option<String>,
    /// 1-based index of the socket group PoB treats as the main skill.
    pub main_socket_group: Option<u32>,
    /// `Build/@targetVersion`, used to tell PoE1 from PoE2 builds.
    pub target_version: Option<String>,
    /// 1-based index of the selected spec (`Tree/@activeSpec`); defaults to 1.
    pub active_spec: usize,
    /// `Skills/@activeSkillSet` - the id (not index) of the selected skill set.
    pub active_skill_set_id: Option<u32>,
    /// Passive-tree snapshots, in document order.
    pub specs: Vec<TreeSpec>,
    /// Skill sets, in document order. Legacy builds with `<Skill>`s directly under `<Skills>` get a
    /// single synthetic set with no id/title.
    pub skill_sets: Vec<SkillSet>,
}

impl Default for PobBuild {
    fn default() -> Self {
        Self {
            level: None,
            class_name: None,
            ascend_class_name: None,
            main_socket_group: None,
            target_version: None,
            active_spec: 1,
            active_skill_set_id: None,
            specs: Vec::new(),
            skill_sets: Vec::new(),
        }
    }
}

impl PobBuild {
    /// The currently-selected spec (`activeSpec`), falling back to the first spec.
    pub fn selected_spec(&self) -> Option<&TreeSpec> {
        self.specs
            .get(self.active_spec.saturating_sub(1))
            .or_else(|| self.specs.first())
    }

    /// The active skill set (matched by `activeSkillSet` id), falling back to the first set.
    pub fn active_skill_set(&self) -> Option<&SkillSet> {
        match self.active_skill_set_id {
            Some(id) => self
                .skill_sets
                .iter()
                .find(|s| s.id == Some(id))
                .or_else(|| self.skill_sets.first()),
            None => self.skill_sets.first(),
        }
    }

    /// The main socket group (`mainSocketGroup`) within the active skill set, else its first group.
    pub fn main_group(&self) -> Option<&SkillGroup> {
        let set = self.active_skill_set()?;
        let idx = self.main_socket_group.unwrap_or(1);
        set.groups
            .get((idx as usize).saturating_sub(1))
            .or_else(|| set.groups.first())
    }

    /// Every gem across every skill set, for build-wide "does this build use gem X" checks.
    pub fn gems(&self) -> impl Iterator<Item = &Gem> {
        self.skill_sets
            .iter()
            .flat_map(|s| s.groups.iter())
            .flat_map(|g| g.gems.iter())
    }
}

/// One passive-tree snapshot. Carries no level/act; the title is the only leveling-stage hint.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreeSpec {
    /// Free-text label, e.g. "Level 12 (Act 2)", "Leaguestart", "Endgame"; often absent.
    pub title: Option<String>,
    /// Game passive-tree data version, e.g. "3_25". NOT a character level.
    pub tree_version: Option<String>,
    /// 0=Scion,1=Marauder,2=Ranger,3=Witch,4=Duelist,5=Templar,6=Shadow.
    pub class_id: Option<u8>,
    pub ascend_class_id: Option<u8>,
    /// Allocated passive node ids (parsed from the `nodes` attribute).
    pub nodes: Vec<u32>,
}

/// A named group of socket groups. `activeSkillSet` selects one by [`SkillSet::id`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillSet {
    pub id: Option<u32>,
    pub title: Option<String>,
    pub groups: Vec<SkillGroup>,
}

/// One socket group (a linked set of gems in an item slot).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillGroup {
    pub enabled: bool,
    /// Item slot, e.g. "Body Armour", "Weapon 1"; `None` when unassigned.
    pub slot: Option<String>,
    /// Set when the skill is granted by an item rather than a socketed gem.
    pub source: Option<String>,
    pub label: Option<String>,
    /// 1-based index of the main active gem within this group.
    pub main_active_skill: Option<u32>,
    pub gems: Vec<Gem>,
}

impl SkillGroup {
    /// The group's headline active gem: the first enabled non-support gem, else the first non-support.
    pub fn first_active_gem(&self) -> Option<&Gem> {
        self.gems
            .iter()
            .find(|g| !g.is_support && g.enabled)
            .or_else(|| self.gems.iter().find(|g| !g.is_support))
    }
}

/// One gem in a socket group.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gem {
    /// Display name, e.g. "Fireball".
    pub name_spec: Option<String>,
    /// Internal skill id, e.g. "Fireball" / "SupportSpellEcho".
    pub skill_id: Option<String>,
    /// Metadata path, e.g. "Metadata/Items/Gems/SkillGemFireball". The gem/quest join key.
    pub gem_id: Option<String>,
    pub level: Option<u32>,
    pub quality: Option<u32>,
    pub enabled: bool,
    /// Whether this is a support gem (derived from `gemId`/`skillId`/`nameSpec`).
    pub is_support: bool,
}
