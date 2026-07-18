//! The build side of the world: the imported Path of Building build and a slim summary of it.
//!
//! [`CampaignState`](crate::state::CampaignState) is the projection of the log; [`BuildState`] is the
//! projection of the imported build. The guidance engine reads both (plus the [`DataPack`]) to decide
//! the next objective. Only [`BuildSummary`] crosses the IPC boundary - the raw [`PobBuild`] stays in
//! the core.

use poebo_pob::PobBuild;

/// Holds the currently-imported build, if any. A missing build is a valid state (guidance still
/// helps with campaign flow), so this defaults to empty.
#[derive(Debug, Clone, Default)]
pub struct BuildState {
    build: Option<PobBuild>,
}

impl BuildState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the loaded build.
    pub fn set(&mut self, build: PobBuild) {
        self.build = Some(build);
    }

    /// Forget the loaded build.
    pub fn clear(&mut self) {
        self.build = None;
    }

    /// The imported build, if one is loaded.
    pub fn build(&self) -> Option<&PobBuild> {
        self.build.as_ref()
    }

    /// A slim, UI-facing summary of the loaded build.
    pub fn summary(&self) -> Option<BuildSummary> {
        self.build.as_ref().map(BuildSummary::from_build)
    }
}

/// A compact snapshot of the imported build for the overlay. Emitted as `build://updated`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
pub struct BuildSummary {
    pub class_name: Option<String>,
    pub ascend_class_name: Option<String>,
    pub level: Option<u32>,
    /// Free-text title of the currently-selected spec (may be absent).
    pub active_spec_title: Option<String>,
    /// The headline active skill of the build's main socket group.
    pub main_skill: Option<String>,
    pub spec_count: u32,
    pub gem_count: u32,
}

impl BuildSummary {
    /// Derive the summary from a parsed build.
    pub fn from_build(build: &PobBuild) -> Self {
        let active_spec_title = build.selected_spec().and_then(|s| s.title.clone());
        let main_skill = build
            .main_group()
            .and_then(|g| g.first_active_gem())
            .and_then(|gem| gem.name_spec.clone());
        BuildSummary {
            class_name: build.class_name.clone(),
            ascend_class_name: build.ascend_class_name.clone(),
            level: build.level,
            active_spec_title,
            main_skill,
            spec_count: build.specs.len() as u32,
            gem_count: build.gems().count() as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poebo_pob::{Gem, PobBuild, SkillGroup, SkillSet, TreeSpec};

    fn gem(name: &str, support: bool) -> Gem {
        Gem {
            name_spec: Some(name.to_string()),
            is_support: support,
            enabled: true,
            ..Gem::default()
        }
    }

    fn sample_build() -> PobBuild {
        PobBuild {
            level: Some(90),
            class_name: Some("Witch".into()),
            ascend_class_name: Some("Necromancer".into()),
            main_socket_group: Some(1),
            active_spec: 2,
            specs: vec![
                TreeSpec {
                    title: Some("Leaguestart".into()),
                    ..TreeSpec::default()
                },
                TreeSpec {
                    title: Some("Level 90 (Endgame)".into()),
                    ..TreeSpec::default()
                },
            ],
            skill_sets: vec![SkillSet {
                id: Some(1),
                groups: vec![SkillGroup {
                    enabled: true,
                    gems: vec![gem("Raise Zombie", false), gem("Spell Echo Support", true)],
                    ..SkillGroup::default()
                }],
                ..SkillSet::default()
            }],
            ..PobBuild::default()
        }
    }

    #[test]
    fn summary_reflects_active_spec_and_main_skill() {
        let mut state = BuildState::new();
        assert!(state.summary().is_none(), "no build -> no summary");
        state.set(sample_build());
        let s = state.summary().expect("summary");
        assert_eq!(s.class_name.as_deref(), Some("Witch"));
        assert_eq!(s.ascend_class_name.as_deref(), Some("Necromancer"));
        assert_eq!(s.level, Some(90));
        assert_eq!(s.active_spec_title.as_deref(), Some("Level 90 (Endgame)"));
        assert_eq!(s.main_skill.as_deref(), Some("Raise Zombie"));
        assert_eq!(s.spec_count, 2);
        assert_eq!(s.gem_count, 2);
    }

    #[test]
    fn clear_forgets_the_build() {
        let mut state = BuildState::new();
        state.set(sample_build());
        assert!(state.build().is_some());
        state.clear();
        assert!(state.build().is_none());
        assert!(state.summary().is_none());
    }
}
