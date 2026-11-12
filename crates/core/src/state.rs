//! The campaign state machine: folds [`LogEvent`]s into a [`CampaignState`] projection.
//!
//! It is deliberately tolerant of real play: logouts, portals to town, hideout visits, repeated
//! zone entries, and party members' events. The hardest correctness concern is that **zone names
//! repeat across Acts 1-5 / 6-10** (see `data`), so [`CampaignMachine::resolve_zone`] disambiguates
//! by, in order: (1) the authoritative `worldAreaId` from a preceding `AreaGenerated` line, then
//! (2) progression context (the act closest to the current one, preferring forward movement), then
//! (3) the sole candidate, else treats the zone as unknown and keeps the last known act.

use std::sync::Arc;

use time::PrimitiveDateTime;

use crate::data::{ZoneInfo, ZoneTable};
use crate::events::{DeathCause, LogEvent};

const STRAND: &str = "1_1_1";

/// Where the player currently is, coarsely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum LocationKind {
    Town,
    Hideout,
    Combat,
    #[default]
    Unknown,
}

/// The resolved current zone. `id`/`act` are `None` when the zone name wasn't found in the pack.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
pub struct ZoneRef {
    pub name: String,
    pub id: Option<String>,
    pub act: Option<u8>,
}

/// The projection of the log the UI renders. Serialized to the frontend as `campaign://state-updated`.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
pub struct CampaignState {
    pub character: Option<String>,
    pub class: Option<String>,
    pub level: u32,
    pub act: Option<u8>,
    pub zone: Option<ZoneRef>,
    pub location_kind: LocationKind,
    pub deaths: u32,
    pub session_started: bool,
    /// Raw id from the `Generating level` line, kept even for areas the pack doesn't know (the
    /// labyrinth). The route tracker keys off this.
    pub area_id: Option<String>,
    /// Areas keep loading but the "You have entered" line stopped coming, as after a patch that
    /// reworded it. The zone then follows the `Generating level` line alone.
    pub log_unrecognised: bool,
    /// When the current zone was entered. Internal (drives future "time in zone"). Not serialized.
    #[serde(skip)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub last_zone_change: Option<PrimitiveDateTime>,
}

/// Which fields changed on an [`CampaignMachine::apply`]. The pipeline only emits when meaningful.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StateDelta {
    pub zone_changed: bool,
    pub act_changed: bool,
    pub level_changed: bool,
    pub deaths_changed: bool,
    pub character_changed: bool,
    pub location_changed: bool,
    pub session_changed: bool,
    pub log_changed: bool,
}

impl StateDelta {
    /// Whether anything the UI cares about changed.
    pub fn is_meaningful(&self) -> bool {
        self.zone_changed
            || self.act_changed
            || self.level_changed
            || self.deaths_changed
            || self.character_changed
            || self.location_changed
            || self.session_changed
            || self.log_changed
    }
}

/// Folds log events into a [`CampaignState`].
#[derive(Debug, Clone)]
pub struct CampaignMachine {
    state: CampaignState,
    zones: Arc<ZoneTable>,
    /// worldAreaId from the most recent `AreaGenerated`, consumed by the next `ZoneEntered`.
    last_generated_id: Option<String>,
    /// Instance connections and generated areas since the last `ZoneEntered`.
    loads_without_entry: (u32, u32),
}

/// Area loads in a row without a "You have entered" line before the log counts as unrecognised.
/// One can go missing in normal play (a crash while loading), three don't.
const UNRECOGNISED_AFTER: u32 = 3;

impl CampaignMachine {
    pub fn new(zones: Arc<ZoneTable>) -> Self {
        Self {
            state: CampaignState::default(),
            zones,
            last_generated_id: None,
            loads_without_entry: (0, 0),
        }
    }

    /// Borrow the current state (e.g. to serialize for the UI).
    pub fn snapshot(&self) -> &CampaignState {
        &self.state
    }

    /// Consume the machine, yielding the final state.
    pub fn into_state(self) -> CampaignState {
        self.state
    }

    /// Apply one event, returning which fields changed.
    pub fn apply(&mut self, event: LogEvent, at: Option<PrimitiveDateTime>) -> StateDelta {
        match event {
            LogEvent::SessionStart => self.apply_session_start(),
            LogEvent::ZoneEntered { zone } => {
                self.loads_without_entry = (0, 0);
                let recognised = self.set_unrecognised(false);
                let delta = self.apply_zone_entered(&zone, at);
                StateDelta {
                    log_changed: recognised,
                    ..delta
                }
            }
            LogEvent::LevelUp {
                character,
                class,
                level,
            } => self.apply_level_up(character, class, level),
            LogEvent::Death { name, cause } => self.apply_death(&name, cause),
            LogEvent::InstanceConnect { .. } => {
                self.loads_without_entry.0 += 1;
                self.check_loads()
            }
            LogEvent::AreaGenerated { world_area_id, .. } => {
                self.loads_without_entry.1 += 1;
                let log_changed = self.check_loads().log_changed;
                self.last_generated_id = Some(world_area_id);
                let delta = if self.state.log_unrecognised {
                    self.enter_generated(at)
                } else {
                    StateDelta::default()
                };
                StateDelta {
                    log_changed,
                    ..delta
                }
            }
        }
    }

    fn check_loads(&mut self) -> StateDelta {
        let (connects, generated) = self.loads_without_entry;
        StateDelta {
            log_changed: connects.max(generated) >= UNRECOGNISED_AFTER
                && self.set_unrecognised(true),
            ..Default::default()
        }
    }

    /// Whether the flag changed.
    fn set_unrecognised(&mut self, on: bool) -> bool {
        let changed = self.state.log_unrecognised != on;
        self.state.log_unrecognised = on;
        changed
    }

    /// Enters the area the last `AreaGenerated` named, for a log without zone lines. An area the
    /// pack doesn't know (a hideout, the labyrinth) still moves the tracker by its id.
    fn enter_generated(&mut self, at: Option<PrimitiveDateTime>) -> StateDelta {
        let id = self.last_generated_id.clone().unwrap_or_default();
        let name = self
            .zones
            .by_id(&id)
            .map_or_else(|| id.clone(), |z| z.name.clone());
        self.apply_zone_entered(&name, at)
    }

    fn apply_session_start(&mut self) -> StateDelta {
        self.last_generated_id = None;
        let changed = !self.state.session_started;
        self.state.session_started = true;
        StateDelta {
            session_changed: changed,
            ..Default::default()
        }
    }

    fn apply_zone_entered(&mut self, zone_name: &str, at: Option<PrimitiveDateTime>) -> StateDelta {
        let resolved: Option<ZoneInfo> = self.resolve_zone(zone_name).cloned();
        // The AreaGenerated hint is consumed by exactly one zone change.
        let generated_id = self.last_generated_id.take();
        let old_area_id = self.state.area_id.clone();

        let old_key = self
            .state
            .zone
            .as_ref()
            .map(|z| (z.id.clone(), z.name.clone()));
        let old_act = self.state.act;
        let old_loc = self.state.location_kind;

        let (zref, act, loc) = match &resolved {
            Some(z) => {
                let act = z.act as u8;
                let loc = if z.is_town {
                    LocationKind::Town
                } else if z.is_hideout {
                    LocationKind::Hideout
                } else {
                    LocationKind::Combat
                };
                (
                    ZoneRef {
                        name: z.name.clone(),
                        id: Some(z.id.clone()),
                        act: Some(act),
                    },
                    Some(act),
                    loc,
                )
            }
            None => {
                // Unknown zone (hideout, side area, logout screen): keep the last known act.
                let loc = if zone_name.contains("Hideout") {
                    LocationKind::Hideout
                } else {
                    LocationKind::Unknown
                };
                (
                    ZoneRef {
                        name: zone_name.to_string(),
                        id: None,
                        act: self.state.act,
                    },
                    self.state.act,
                    loc,
                )
            }
        };

        let new_key = Some((zref.id.clone(), zref.name.clone()));
        self.state.area_id = generated_id;
        let zone_changed = old_key != new_key || old_area_id != self.state.area_id;
        if zone_changed {
            self.state.last_zone_change = at;
        }
        self.state.zone = Some(zref);
        self.state.act = act;
        self.state.location_kind = loc;

        StateDelta {
            zone_changed,
            act_changed: old_act != act,
            location_changed: old_loc != loc,
            ..Default::default()
        }
    }

    /// Resolve a (possibly ambiguous) zone name to a single area. See the module docs for the order.
    fn resolve_zone(&self, zone_name: &str) -> Option<&ZoneInfo> {
        // 1. Authoritative: the worldAreaId from a preceding AreaGenerated, if its name matches.
        if let Some(id) = &self.last_generated_id {
            if let Some(z) = self.zones.by_id(id) {
                if z.name.eq_ignore_ascii_case(zone_name) {
                    return Some(z);
                }
            }
        }

        // 2/3. Name candidates, disambiguated by progression (closest act, prefer forward).
        let candidates = self.zones.zones_named(zone_name);
        let cur = self.state.act.map(i32::from);
        candidates.iter().min_by_key(|z| match cur {
            Some(c) => ((z.act - c).abs(), i32::from(z.act < c)),
            None => (z.act, 0),
        })
    }

    fn apply_level_up(&mut self, character: String, class: String, level: u32) -> StateDelta {
        // Another name levelling up is a party member, unless it happens on the Twilight Strand:
        // that's a new character of ours, such as the real one after a mule. The log has no
        // line for logging in as another character.
        let other = self
            .state
            .character
            .as_ref()
            .is_some_and(|cur| cur != &character);
        let new_character = other && self.state.area_id.as_deref() == Some(STRAND);
        if other && !new_character {
            return StateDelta::default();
        }
        let character_changed = self.state.character.as_deref() != Some(character.as_str())
            || self.state.class.as_deref() != Some(class.as_str());
        self.state.character = Some(character);
        self.state.class = Some(class);
        let level_changed =
            level > self.state.level || (new_character && level != self.state.level);
        if level_changed {
            self.state.level = level;
        }
        StateDelta {
            level_changed,
            character_changed,
            ..Default::default()
        }
    }

    fn apply_death(&mut self, name: &str, _cause: DeathCause) -> StateDelta {
        // Only count our own character's deaths (the message fires for any player in the instance).
        match &self.state.character {
            Some(c) if c == name => {
                self.state.deaths += 1;
                StateDelta {
                    deaths_changed: true,
                    ..Default::default()
                }
            }
            _ => StateDelta::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::LogEvent;

    // A tiny table with a cross-act name collision: "Riverside" exists in Act 1 and Act 6.
    const PACK: &str = r#"{
        "schema_version": 1, "patch": "t", "generated_at": "t", "source": "t",
        "areas": [
            {"id":"1_1_town","name":"Lioneye's Watch","act":1,"area_level":1,
             "is_town":true,"is_hideout":false,"is_map":false,"has_waypoint":true,"connections":[]},
            {"id":"1_1_1","name":"The Twilight Strand","act":1,"area_level":1,
             "is_town":false,"is_hideout":false,"is_map":false,"has_waypoint":false,"connections":[]},
            {"id":"1_1_2","name":"Riverside","act":1,"area_level":2,
             "is_town":false,"is_hideout":false,"is_map":false,"has_waypoint":true,"connections":[]},
            {"id":"1_5_town","name":"Overseer's Tower","act":5,"area_level":40,
             "is_town":true,"is_hideout":false,"is_map":false,"has_waypoint":true,"connections":[]},
            {"id":"2_6_2","name":"Riverside","act":6,"area_level":45,
             "is_town":false,"is_hideout":false,"is_map":false,"has_waypoint":true,"connections":[]}
        ]
    }"#;

    fn machine() -> CampaignMachine {
        let zones = Arc::new(ZoneTable::from_json_bytes(PACK.as_bytes(), 1).expect("pack"));
        CampaignMachine::new(zones)
    }

    fn enter(m: &mut CampaignMachine, zone: &str) -> StateDelta {
        m.apply(
            LogEvent::ZoneEntered {
                zone: zone.to_string(),
            },
            None,
        )
    }

    #[test]
    fn infers_act_from_zone() {
        let mut m = machine();
        let d = enter(&mut m, "Lioneye's Watch");
        assert!(d.zone_changed && d.act_changed && d.location_changed);
        assert_eq!(m.snapshot().act, Some(1));
        assert_eq!(m.snapshot().location_kind, LocationKind::Town);
    }

    #[test]
    fn unknown_zone_keeps_last_act_and_sets_location() {
        let mut m = machine();
        enter(&mut m, "The Twilight Strand"); // act 1, combat
        assert_eq!(m.snapshot().act, Some(1));
        // Portal to a hideout (not in the pack): act preserved, location becomes Hideout.
        let d = enter(&mut m, "Enlightened Hideout");
        assert_eq!(m.snapshot().act, Some(1), "act preserved on unknown zone");
        assert_eq!(m.snapshot().location_kind, LocationKind::Hideout);
        assert!(d.zone_changed && !d.act_changed);
        // An unknown non-hideout zone -> Unknown location, act still preserved.
        enter(&mut m, "Some Side Area");
        assert_eq!(m.snapshot().act, Some(1));
        assert_eq!(m.snapshot().location_kind, LocationKind::Unknown);
    }

    #[test]
    fn repeat_zone_is_idempotent() {
        let mut m = machine();
        enter(&mut m, "The Twilight Strand");
        let d = enter(&mut m, "The Twilight Strand");
        assert!(!d.zone_changed && !d.act_changed && !d.is_meaningful());
    }

    #[test]
    fn name_collision_resolves_by_progression() {
        let mut m = machine();
        // Early game: entering "Riverside" with no/low act -> Act 1 copy.
        enter(&mut m, "Lioneye's Watch"); // act 1
        enter(&mut m, "Riverside");
        assert_eq!(m.snapshot().act, Some(1));
        assert_eq!(
            m.snapshot().zone.as_ref().unwrap().id.as_deref(),
            Some("1_1_2")
        );

        // Progress to Act 5, then re-enter "Riverside" -> the Act 6 copy (closest forward act).
        enter(&mut m, "Overseer's Tower"); // act 5
        enter(&mut m, "Riverside");
        assert_eq!(m.snapshot().act, Some(6));
        assert_eq!(
            m.snapshot().zone.as_ref().unwrap().id.as_deref(),
            Some("2_6_2")
        );
    }

    #[test]
    fn area_generated_id_is_authoritative_for_disambiguation() {
        let mut m = machine();
        enter(&mut m, "Lioneye's Watch"); // act 1
                                          // Even while in Act 1, an explicit AreaGenerated for the Act 6 Riverside wins.
        m.apply(
            LogEvent::AreaGenerated {
                level: 45,
                world_area_id: "2_6_2".to_string(),
                seed: 1,
            },
            None,
        );
        enter(&mut m, "Riverside");
        assert_eq!(m.snapshot().act, Some(6));
        assert_eq!(
            m.snapshot().zone.as_ref().unwrap().id.as_deref(),
            Some("2_6_2")
        );
    }

    #[test]
    fn level_is_monotonic_and_captures_character() {
        let mut m = machine();
        let d = m.apply(
            LogEvent::LevelUp {
                character: "Xophina".into(),
                class: "Witch".into(),
                level: 12,
            },
            None,
        );
        assert!(d.level_changed && d.character_changed);
        assert_eq!(m.snapshot().level, 12);
        assert_eq!(m.snapshot().character.as_deref(), Some("Xophina"));
        // A stale/out-of-order lower level never lowers the level.
        m.apply(
            LogEvent::LevelUp {
                character: "Xophina".into(),
                class: "Witch".into(),
                level: 10,
            },
            None,
        );
        assert_eq!(m.snapshot().level, 12);
    }

    #[test]
    fn a_new_character_on_the_twilight_strand_takes_over() {
        let mut m = machine();
        let level_up = |m: &mut CampaignMachine, character: &str, class: &str, level| {
            m.apply(
                LogEvent::LevelUp {
                    character: character.into(),
                    class: class.into(),
                    level,
                },
                None,
            )
        };
        level_up(&mut m, "MuleGuy", "Ranger", 4);
        m.apply(
            LogEvent::AreaGenerated {
                level: 1,
                world_area_id: "1_1_1".into(),
                seed: 1,
            },
            None,
        );
        enter(&mut m, "The Twilight Strand");
        let d = level_up(&mut m, "RealOne", "Templar", 2);
        assert!(d.character_changed && d.level_changed);
        let s = m.snapshot();
        assert_eq!(s.character.as_deref(), Some("RealOne"));
        assert_eq!(s.class.as_deref(), Some("Templar"));
        assert_eq!(s.level, 2);
        // Off the Strand, the mule is the other player.
        m.apply(
            LogEvent::AreaGenerated {
                level: 1,
                world_area_id: "1_1_town".into(),
                seed: 2,
            },
            None,
        );
        enter(&mut m, "Lioneye's Watch");
        assert!(!level_up(&mut m, "MuleGuy", "Ranger", 5).is_meaningful());
        assert_eq!(m.snapshot().character.as_deref(), Some("RealOne"));
    }

    #[test]
    fn party_member_levelups_and_deaths_are_ignored() {
        let mut m = machine();
        m.apply(
            LogEvent::LevelUp {
                character: "Xophina".into(),
                class: "Witch".into(),
                level: 20,
            },
            None,
        );
        // Party member levels up - must not touch our level.
        let d = m.apply(
            LogEvent::LevelUp {
                character: "SomeFriend".into(),
                class: "Marauder".into(),
                level: 55,
            },
            None,
        );
        assert!(!d.is_meaningful());
        assert_eq!(m.snapshot().level, 20);
        // Party member dies - not our death.
        let d = m.apply(
            LogEvent::Death {
                name: "SomeFriend".into(),
                cause: DeathCause::Slain,
            },
            None,
        );
        assert!(!d.deaths_changed);
        assert_eq!(m.snapshot().deaths, 0);
        // Our death counts.
        let d = m.apply(
            LogEvent::Death {
                name: "Xophina".into(),
                cause: DeathCause::Slain,
            },
            None,
        );
        assert!(d.deaths_changed);
        assert_eq!(m.snapshot().deaths, 1);
    }

    #[test]
    fn session_start_flags_once() {
        let mut m = machine();
        assert!(m.apply(LogEvent::SessionStart, None).session_changed);
        assert!(m.snapshot().session_started);
        assert!(!m.apply(LogEvent::SessionStart, None).session_changed);
    }

    fn load(m: &mut CampaignMachine, id: &str) -> StateDelta {
        m.apply(
            LogEvent::InstanceConnect {
                server: "1.2.3.4:6112".into(),
            },
            None,
        );
        m.apply(
            LogEvent::AreaGenerated {
                level: 1,
                world_area_id: id.into(),
                seed: 1,
            },
            None,
        )
    }

    #[test]
    fn follows_generated_areas_when_zone_lines_stop() {
        let mut m = machine();
        load(&mut m, "1_1_1");
        enter(&mut m, "The Twilight Strand");
        assert!(!load(&mut m, "1_1_town").is_meaningful());
        assert!(!load(&mut m, "1_1_2").is_meaningful());
        assert!(!m.snapshot().log_unrecognised);

        let d = load(&mut m, "1_1_town");
        assert!(d.zone_changed);
        assert!(m.snapshot().log_unrecognised);
        assert_eq!(m.snapshot().area_id.as_deref(), Some("1_1_town"));
        assert_eq!(m.snapshot().location_kind, LocationKind::Town);

        let d = load(&mut m, "2_6_2");
        assert!(d.zone_changed && !d.log_changed);
        assert_eq!(m.snapshot().act, Some(6));
    }

    #[test]
    fn a_zone_line_makes_the_log_recognised_again() {
        let mut m = machine();
        let connect = LogEvent::InstanceConnect {
            server: "1.2.3.4:6112".into(),
        };
        assert!(!m.apply(connect.clone(), None).log_changed);
        assert!(!m.apply(connect.clone(), None).log_changed);
        assert!(m.apply(connect, None).log_changed);
        assert!(m.snapshot().log_unrecognised);
        let d = enter(&mut m, "Lioneye's Watch");
        assert!(d.log_changed);
        assert!(!m.snapshot().log_unrecognised);
    }

    #[test]
    fn one_load_without_a_zone_line_is_normal() {
        let mut m = machine();
        for _ in 0..5 {
            load(&mut m, "1_1_2");
            load(&mut m, "1_1_2");
            enter(&mut m, "Riverside");
        }
        assert!(!m.snapshot().log_unrecognised);
    }
}
