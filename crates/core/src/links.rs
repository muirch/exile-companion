//! A skill set of the build drawn the way build sites draw it: socket groups by slot, each gem with
//! its colour and where the campaign gives it.

use exile_pob::PobBuild;
use serde::Serialize;

use crate::gemplan::GemPlan;
use crate::gems::GemTable;
use crate::resolve::stage_for_level;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GemColor {
    Red,
    Green,
    Blue,
    White,
}

impl GemColor {
    fn of(attribute: &str) -> Self {
        match attribute {
            "strength" => Self::Red,
            "dexterity" => Self::Green,
            "intelligence" => Self::Blue,
            _ => Self::White,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkGem {
    pub name: String,
    pub color: GemColor,
    pub support: bool,
    /// As the build sets them in PoB.
    pub level: Option<u32>,
    pub quality: Option<u32>,
    pub required_level: Option<u32>,
    /// "Quest reward from Tarkleigh in act 1", a mule, or not in the campaign.
    pub get: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkGroup {
    pub slot: Option<String>,
    pub label: Option<String>,
    pub gems: Vec<LinkGem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Links {
    /// Every skill set's title, in order.
    pub sets: Vec<String>,
    /// The one shown, an index into `sets`.
    pub set: usize,
    pub groups: Vec<LinkGroup>,
}

fn set_titles(build: &PobBuild) -> Vec<String> {
    build
        .skill_sets
        .iter()
        .enumerate()
        .map(|(i, s)| s.title.clone().unwrap_or_else(|| format!("Set {}", i + 1)))
        .collect()
}

/// The skill set a character of `level` is at: the one whose title names the highest level not
/// above it, else the one the build has active.
pub fn set_for_level(build: &PobBuild, level: u32) -> usize {
    let titles = set_titles(build);
    stage_for_level(titles.iter().map(String::as_str), level).unwrap_or_else(|| {
        build
            .active_skill_set_id
            .and_then(|id| build.skill_sets.iter().position(|s| s.id == Some(id)))
            .unwrap_or(0)
    })
}

fn get(name: &str, plan: Option<&GemPlan>) -> Option<String> {
    let plan = plan?;
    if let Some(step) = plan.steps.iter().find(|s| s.name == name) {
        return Some(step.place());
    }
    let mule = plan
        .mules
        .iter()
        .find(|m| m.take.iter().chain(&m.buy).any(|g| g.name == name));
    if let Some(m) = mule {
        return Some(format!("From the {} mule", m.class));
    }
    plan.not_in_campaign
        .iter()
        .any(|n| n == name)
        .then(|| "Not in the campaign".to_string())
}

pub fn links(build: &PobBuild, set: usize, gems: &GemTable, plan: Option<&GemPlan>) -> Links {
    let sets = set_titles(build);
    let set = set.min(sets.len().saturating_sub(1));
    let groups = build
        .skill_sets
        .get(set)
        .map(|s| s.groups.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|g| g.enabled && g.source.is_none())
        .map(|g| LinkGroup {
            slot: g.slot.clone(),
            label: g.label.clone().filter(|l| !l.trim().is_empty()),
            gems: g
                .gems
                .iter()
                .filter(|gem| gem.enabled)
                .filter_map(|gem| {
                    let info = gem.gem_id.as_deref().and_then(|id| gems.gem(id));
                    let name = info
                        .map(|i| i.name.clone())
                        .or_else(|| gem.name_spec.clone())?;
                    Some(LinkGem {
                        get: get(&name, plan),
                        color: GemColor::of(info.map_or("", |i| i.primary_attribute.as_str())),
                        support: gem.is_support,
                        level: gem.level,
                        quality: gem.quality,
                        required_level: info.map(|i| i.required_level),
                        name,
                    })
                })
                .collect(),
        })
        .filter(|g: &LinkGroup| !g.gems.is_empty())
        .collect();
    Links { sets, set, groups }
}

#[cfg(test)]
mod tests {
    use exile_pob::{Gem, SkillGroup, SkillSet};

    use super::*;
    use crate::gemplan::{GemAction, GemStep};

    const GEMS: &str = r#"{
      "Metadata/Items/Gems/SkillGemSunder": {"id": "Metadata/Items/Gems/SkillGemSunder", "name": "Sunder", "primary_attribute": "strength", "required_level": 12, "is_support": false},
      "Metadata/Items/Gems/SupportGemRuthless": {"id": "Metadata/Items/Gems/SupportGemRuthless", "name": "Ruthless Support", "primary_attribute": "strength", "required_level": 8, "is_support": true},
      "Metadata/Items/Gems/SkillGemPrecision": {"id": "Metadata/Items/Gems/SkillGemPrecision", "name": "Precision", "primary_attribute": "dexterity", "required_level": 1, "is_support": false}
    }"#;

    fn gem(id: &str, name: &str, support: bool) -> Gem {
        Gem {
            name_spec: Some(name.into()),
            skill_id: None,
            gem_id: Some(format!("Metadata/Items/Gems/{id}")),
            level: Some(20),
            quality: Some(0),
            enabled: true,
            is_support: support,
        }
    }

    fn build() -> PobBuild {
        let set = |title: &str, groups| SkillSet {
            title: Some(title.into()),
            groups,
            ..SkillSet::default()
        };
        let group = |slot: &str, gems| SkillGroup {
            enabled: true,
            slot: Some(slot.into()),
            source: None,
            label: None,
            main_active_skill: Some(1),
            gems,
        };
        PobBuild {
            skill_sets: vec![
                set(
                    "Level 1-12",
                    vec![group(
                        "Gloves",
                        vec![gem("SkillGemPrecision", "Precision", false)],
                    )],
                ),
                set(
                    "Level 12-22",
                    vec![
                        group(
                            "Body Armour",
                            vec![
                                gem("SkillGemSunder", "Sunder", false),
                                gem("SupportGemRuthless", "Ruthless", true),
                            ],
                        ),
                        SkillGroup {
                            enabled: false,
                            ..group("Helmet", vec![gem("SkillGemPrecision", "Precision", false)])
                        },
                    ],
                ),
            ],
            ..PobBuild::default()
        }
    }

    #[test]
    fn picks_the_set_for_the_level() {
        assert_eq!(set_for_level(&build(), 5), 0);
        assert_eq!(set_for_level(&build(), 14), 1);
    }

    #[test]
    fn draws_the_groups_with_colours_and_where_to_get_each_gem() {
        let table = GemTable::from_json_bytes(GEMS.as_bytes(), b"{}").unwrap();
        let plan = GemPlan {
            steps: vec![GemStep {
                gem_id: "Metadata/Items/Gems/SkillGemSunder".into(),
                name: "Sunder".into(),
                quest_id: "a1q4".into(),
                act: 1,
                action: GemAction::Buy,
                npc: "Nessa".into(),
                need_level: 12,
            }],
            not_in_campaign: vec!["Ruthless Support".into()],
            ..GemPlan::default()
        };
        let l = links(&build(), 1, &table, Some(&plan));
        assert_eq!(l.sets, ["Level 1-12", "Level 12-22"]);
        assert_eq!(l.set, 1);
        assert_eq!(l.groups.len(), 1, "a disabled group is left out");
        let body = &l.groups[0];
        assert_eq!(body.slot.as_deref(), Some("Body Armour"));
        assert_eq!(body.gems[0].name, "Sunder");
        assert_eq!(body.gems[0].color, GemColor::Red);
        assert_eq!(body.gems[0].get.as_deref(), Some("Sold by Nessa in act 1"));
        assert_eq!(body.gems[1].name, "Ruthless Support");
        assert!(body.gems[1].support);
        assert_eq!(body.gems[1].get.as_deref(), Some("Not in the campaign"));
        assert_eq!(
            links(&build(), 0, &table, None).groups[0].gems[0].color,
            GemColor::Green
        );
        // A set past the end falls back to the last.
        assert_eq!(links(&build(), 9, &table, None).set, 1);
    }
}
