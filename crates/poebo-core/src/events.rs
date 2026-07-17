//! Typed events decoded from `Client.txt` lines.
//!
//! The parser turns a raw line into an optional [`RawLine`] (timestamp + body) and then classifies
//! the body into a [`LogEvent`]. The state machine consumes [`LogEvent`]s; replay pacing uses the
//! timestamp carried alongside on [`TimedEvent`].

use time::PrimitiveDateTime;

/// A structured event decoded from a single `Client.txt` line.
///
/// Serialized with an internal `kind` tag so the frontend can discriminate variants.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LogEvent {
    /// `***** LOG FILE OPENING *****` — a new client session began.
    SessionStart,

    /// `: You have entered <zone>.`
    ZoneEntered { zone: String },

    /// `: <character> (<class>) is now level <level>`
    LevelUp {
        character: String,
        class: String,
        level: u32,
    },

    /// `: <name> has been slain.` / `: <name> has committed suicide.`
    ///
    /// Note: this fires for *any* player in the instance (party members too); the state machine
    /// attributes it to the local character by name.
    Death { name: String, cause: DeathCause },

    /// `Connecting to instance server at <server>` — emitted on every instance load.
    InstanceConnect { server: String },

    /// `Generating level <level> area "<world_area_id>" with seed <seed>`.
    ///
    /// `world_area_id` is the language-independent internal id (e.g. `1_1_town`); captured now as
    /// the join key for the Phase 2 gem/quest resolvers even though Phase 1 keys off zone names.
    AreaGenerated {
        level: u32,
        world_area_id: String,
        seed: u64,
    },
}

/// How a character died, per the two `Client.txt` death phrasings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum DeathCause {
    /// `has been slain.`
    Slain,
    /// `has committed suicide.` (e.g. `/kill`, some reflect/degen deaths)
    Suicide,
}

/// A decoded log line before classification: the parsed timestamp (if present), the message body
/// (everything after `] `), and the original raw line (preserved for recording).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawLine {
    /// Log timestamp in the client's local time (the log carries no timezone, so this is a naive
    /// datetime); `None` if the line had no parseable timestamp. Replay pacing uses deltas, for
    /// which the missing offset cancels out.
    pub ts: Option<PrimitiveDateTime>,
    /// The message body — the classifier's input.
    pub body: String,
    /// The exact original line (without the trailing newline). Recording writes this verbatim.
    pub raw: String,
}

/// A classified event paired with the log timestamp that drives replay pacing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedEvent {
    pub at: Option<PrimitiveDateTime>,
    pub event: LogEvent,
}
