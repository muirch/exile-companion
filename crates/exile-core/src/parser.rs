//! Turns raw `Client.txt` lines into typed [`LogEvent`]s.
//!
//! Two stages:
//! 1. [`split_line`] - strip the `YYYY/MM/DD HH:MM:SS <uptime> <hex> [INFO Client <id>] ` framing to
//!    the message body, parsing the timestamp. Robust rule: parse the fixed 19-char timestamp, then
//!    take everything after the first `"] "`. The one framing without `"] "` is the session-start
//!    banner, handled specially.
//! 2. [`classify`] - map a body to a [`LogEvent`], or `None` for chat/whisper/NPC/unknown noise.
//!    **System events are exactly the bodies with a leading `": "`.** Chat is `Name: msg` or begins
//!    with a channel symbol (`@ # $ % & <`) and thus never starts with `": "`, so it falls through
//!    to `None`. Two no-colon system lines (`Connecting...`, `Generating...`) are matched explicitly.
//!
//! Everything returns `Option`. Nothing panics on malformed input.

use std::sync::LazyLock;

use regex::Regex;
use time::macros::format_description;
use time::PrimitiveDateTime;

use crate::events::{DeathCause, LogEvent, RawLine, TimedEvent};
use crate::locale::LocalePack;

/// `<char> (<class>) is now level <N>` - char names have no spaces. Class is a single token.
static LEVEL_UP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\S+) \(([^)]+)\) is now level (\d+)$").expect("valid regex"));

/// `Generating level <N> area "<id>" with seed <seed>`.
static AREA_GENERATED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^Generating level (\d+) area "([^"]+)" with seed (\d+)$"#).expect("valid regex")
});

const SESSION_MARKER: &str = "LOG FILE OPENING";
const CONNECT_PREFIX: &str = "Connecting to instance server at ";

/// Split a raw log line (without trailing newline) into a timestamp + message body.
///
/// Returns `None` only for an empty/whitespace line. The timestamp is `None` when the line has no
/// parseable `YYYY/MM/DD HH:MM:SS` prefix (the body is still extracted so classification proceeds).
pub fn split_line(line: &str) -> Option<RawLine> {
    if line.trim().is_empty() {
        return None;
    }

    let ts = parse_timestamp(line);

    // The normal framing ends with "] " right after "[INFO Client <id>]".
    let body = if let Some(idx) = line.find("] ") {
        &line[idx + 2..]
    } else if line.contains(SESSION_MARKER) {
        // Session banner: `<ts> ***** LOG FILE OPENING *****` - no bracketed prefix.
        line.get(20..).unwrap_or(line)
    } else {
        // Unframed line we don't recognize. Hand the post-timestamp remainder to the classifier.
        line.get(20..).unwrap_or(line)
    };

    Some(RawLine {
        ts,
        body: body.to_string(),
        raw: line.to_string(),
    })
}

/// Parse the fixed 19-char `YYYY/MM/DD HH:MM:SS` prefix, if present.
fn parse_timestamp(line: &str) -> Option<PrimitiveDateTime> {
    const FMT: &[time::format_description::FormatItem<'_>] =
        format_description!("[year]/[month]/[day] [hour]:[minute]:[second]");
    let head = line.get(..19)?;
    PrimitiveDateTime::parse(head, FMT).ok()
}

/// Classify a message body into a [`LogEvent`]; `None` for chat/whisper/NPC/unknown lines.
pub fn classify(body: &str, locale: &LocalePack) -> Option<LogEvent> {
    // Session banner can arrive with or without a colon depending on framing.
    if body.contains(SESSION_MARKER) {
        return Some(LogEvent::SessionStart);
    }

    // System events (zone/level/death/join) all carry a leading ": ".
    if let Some(rest) = body.strip_prefix(": ") {
        return classify_system(rest, locale);
    }

    // Two system lines have no leading colon.
    if let Some(server) = body.strip_prefix(CONNECT_PREFIX) {
        return Some(LogEvent::InstanceConnect {
            server: server.to_string(),
        });
    }
    if let Some(caps) = AREA_GENERATED.captures(body) {
        let level = caps.get(1)?.as_str().parse().ok()?;
        let world_area_id = caps.get(2)?.as_str().to_string();
        let seed = caps.get(3)?.as_str().parse().ok()?;
        return Some(LogEvent::AreaGenerated {
            level,
            world_area_id,
            seed,
        });
    }

    // Anything else (chat `Name: ...`, whispers `@ # $ % &`, `<Guild> Name:`, NPC dialogue) is noise.
    None
}

/// Classify the part of a system-event body after the leading ": ".
fn classify_system(rest: &str, locale: &LocalePack) -> Option<LogEvent> {
    // Zone entry.
    if let Some(after) = rest.strip_prefix(locale.entered_prefix) {
        if let Some(zone) = after.strip_suffix(locale.entered_suffix) {
            return Some(LogEvent::ZoneEntered {
                zone: zone.to_string(),
            });
        }
    }

    // Death (check before level-up: no overlap, but explicit).
    if let Some(name) = rest.strip_suffix(locale.slain_suffix) {
        return Some(LogEvent::Death {
            name: name.to_string(),
            cause: DeathCause::Slain,
        });
    }
    if let Some(name) = rest.strip_suffix(locale.suicide_suffix) {
        return Some(LogEvent::Death {
            name: name.to_string(),
            cause: DeathCause::Suicide,
        });
    }

    // Level up.
    if let Some(caps) = LEVEL_UP.captures(rest) {
        let character = caps.get(1)?.as_str().to_string();
        let class = caps.get(2)?.as_str().to_string();
        let level = caps.get(3)?.as_str().parse().ok()?;
        return Some(LogEvent::LevelUp {
            character,
            class,
            level,
        });
    }

    // Other system lines (`... has joined the area.`, `... has left the area.`) are not tracked.
    None
}

/// Convenience: split + classify a raw line into a [`TimedEvent`], preserving the timestamp.
///
/// Returns `None` when the line is empty or classifies as noise.
pub fn parse_line(line: &str, locale: &LocalePack) -> Option<TimedEvent> {
    let raw = split_line(line)?;
    let event = classify(&raw.body, locale)?;
    Some(TimedEvent { at: raw.ts, event })
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    const EN: LocalePack = LocalePack::EN;

    fn ev(line: &str) -> Option<LogEvent> {
        let raw = split_line(line)?;
        classify(&raw.body, &EN)
    }

    #[test]
    fn parses_timestamp_and_body() {
        let raw = split_line(
            "2018/05/21 17:52:23 157234265 9b0 [INFO Client 1012] : You have entered Toxic Sewer.",
        )
        .expect("split");
        assert_eq!(raw.ts, Some(datetime!(2018-05-21 17:52:23)));
        assert_eq!(raw.body, ": You have entered Toxic Sewer.");
    }

    #[test]
    fn zone_entered() {
        assert_eq!(
            ev("2018/05/21 17:52:23 157234265 9b0 [INFO Client 1012] : You have entered Lioneye's Watch."),
            Some(LogEvent::ZoneEntered {
                zone: "Lioneye's Watch".to_string()
            })
        );
    }

    #[test]
    fn level_up() {
        assert_eq!(
            ev("2018/05/21 17:52:23 1 9b0 [INFO Client 1012] : Xophina (Witch) is now level 12"),
            Some(LogEvent::LevelUp {
                character: "Xophina".to_string(),
                class: "Witch".to_string(),
                level: 12
            })
        );
    }

    #[test]
    fn deaths() {
        assert_eq!(
            ev("2018/05/21 17:52:23 1 9b0 [INFO Client 1012] : Xophina has been slain."),
            Some(LogEvent::Death {
                name: "Xophina".to_string(),
                cause: DeathCause::Slain
            })
        );
        assert_eq!(
            ev("2018/05/21 17:52:23 1 9b0 [INFO Client 1012] : Xophina has committed suicide."),
            Some(LogEvent::Death {
                name: "Xophina".to_string(),
                cause: DeathCause::Suicide
            })
        );
    }

    #[test]
    fn session_start_banner_without_bracket() {
        assert_eq!(
            ev("2018/05/13 16:05:37 ***** LOG FILE OPENING *****"),
            Some(LogEvent::SessionStart)
        );
    }

    #[test]
    fn instance_connect() {
        assert_eq!(
            ev("2018/05/21 17:52:13 1 d8 [INFO Client 1012] Connecting to instance server at 158.85.40.125:6112"),
            Some(LogEvent::InstanceConnect {
                server: "158.85.40.125:6112".to_string()
            })
        );
    }

    #[test]
    fn area_generated_captures_world_area_id() {
        assert_eq!(
            ev(
                r#"2022/09/29 20:12:53 1 9b0 [INFO Client 1012] Generating level 70 area "MapWorldsGrotto" with seed 2049423767"#
            ),
            Some(LogEvent::AreaGenerated {
                level: 70,
                world_area_id: "MapWorldsGrotto".to_string(),
                seed: 2049423767
            })
        );
    }

    #[test]
    fn drops_chat_and_whisper_noise() {
        let noise = [
            r#"2018/05/21 17:52:23 1 9b0 [INFO Client 1012] @From Buyer: Hi, I would like to buy your Tabula"#,
            r#"2018/05/21 17:52:23 1 9b0 [INFO Client 1012] #GlobalGuy: hello trade chat"#,
            r#"2018/05/21 17:52:23 1 9b0 [INFO Client 1012] $TradeGuy: wtb mirror"#,
            r#"2018/05/21 17:52:23 1 9b0 [INFO Client 1012] %PartyMate: invite"#,
            r#"2018/05/21 17:52:23 1 9b0 [INFO Client 1012] Oshabi: Put down the spawn."#,
            r#"2018/05/21 17:52:23 1 9b0 [INFO Client 1012] <GUILD> SomePlayer: gg"#,
            r#"2018/05/21 17:52:23 1 9b0 [INFO Client 1012] : SomePlayer has joined the area."#,
            r#"2018/05/21 17:52:23 1 9b0 [INFO Client 1012] : SomePlayer has left the area."#,
        ];
        for line in noise {
            assert_eq!(ev(line), None, "should drop noise: {line}");
        }
    }

    #[test]
    fn empty_line_yields_no_rawline() {
        assert!(split_line("   ").is_none());
    }

    #[test]
    fn malformed_timestamp_still_extracts_body() {
        // Corrupt timestamp, but the bracketed body is intact.
        let raw =
            split_line("garbage-not-a-date 1 9b0 [INFO Client 1012] : You have entered The Coast.")
                .expect("split");
        assert_eq!(raw.ts, None);
        assert_eq!(
            classify(&raw.body, &EN),
            Some(LogEvent::ZoneEntered {
                zone: "The Coast".to_string()
            })
        );
    }
}
