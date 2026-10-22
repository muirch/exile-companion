//! Integration tests for the source -> pipeline layer.
//!
//! The headline test proves replay is behaviorally identical to folding the parser directly, so the
//! `EventSource` seam holds. Others exercise pause/step semantics, recorder round-tripping, and the
//! live tailer picking up appended lines.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::StreamExt;
use tokio::io::AsyncWriteExt;

use exile_core::clock::InstantSleeper;
use exile_core::data::ZoneTable;
use exile_core::locale::LocalePack;
use exile_core::parser::parse_line;
use exile_core::pipeline::run_pipeline;
use exile_core::source::{EventSource, LiveTail, ReplayControl, ReplaySource, SpeedMultiplier};
use exile_core::state::{CampaignMachine, CampaignState};

const PACK: &str = r#"{
    "schema_version": 1, "patch": "t", "generated_at": "t", "source": "t",
    "areas": [
        {"id":"1_1_town","name":"Lioneye's Watch","act":1,"area_level":1,
         "is_town":true,"is_hideout":false,"is_map":false,"has_waypoint":true,"connections":[]},
        {"id":"1_1_1","name":"The Twilight Strand","act":1,"area_level":1,
         "is_town":false,"is_hideout":false,"is_map":false,"has_waypoint":false,"connections":[]},
        {"id":"1_1_2","name":"The Coast","act":1,"area_level":2,
         "is_town":false,"is_hideout":false,"is_map":false,"has_waypoint":true,"connections":[]}
    ]
}"#;

const LOG: &str = "\
2018/05/13 16:05:37 ***** LOG FILE OPENING *****\r
2018/05/13 16:05:40 1 d8 [INFO Client 1012] Connecting to instance server at 1.2.3.4:6112\r
2018/05/13 16:05:41 2 9b0 [INFO Client 1012] : You have entered Lioneye's Watch.\r
2018/05/13 16:05:42 3 9b0 [INFO Client 1012] $TradeGuy: wtb mirror cheap\r
2018/05/13 16:05:50 4 9b0 [INFO Client 1012] : Xophina (Witch) is now level 3\r
2018/05/13 16:06:10 5 9b0 [INFO Client 1012] : You have entered The Twilight Strand.\r
2018/05/13 16:07:00 6 9b0 [INFO Client 1012] : Xophina has been slain.\r
2018/05/13 16:07:30 7 9b0 [INFO Client 1012] : You have entered The Coast.\r
";

fn zones() -> Arc<ZoneTable> {
    Arc::new(ZoneTable::from_json_bytes(PACK.as_bytes(), 1).expect("pack"))
}

/// Fold the parser directly over the log - the reference the pipeline must reproduce.
fn reference_state() -> CampaignState {
    let mut m = CampaignMachine::new(zones());
    for line in LOG.lines() {
        if let Some(ev) = parse_line(line, &LocalePack::EN) {
            m.apply(ev.event, ev.at);
        }
    }
    m.into_state()
}

async fn write_fixture(content: &str) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().expect("temp file");
    tokio::fs::write(file.path(), content).await.expect("write");
    file
}

#[tokio::test]
async fn replay_matches_direct_parse() {
    let fixture = write_fixture(LOG).await;
    let ctl = ReplayControl::new(SpeedMultiplier::X100);
    let source = ReplaySource::new(fixture.path(), ctl.clone(), InstantSleeper);

    let final_state = Arc::new(Mutex::new(CampaignState::default()));
    let sink = final_state.clone();
    run_pipeline(
        Box::new(source).into_stream(),
        CampaignMachine::new(zones()),
        None,
        LocalePack::EN,
        move |state, _delta| {
            *sink.lock().unwrap() = state.clone();
        },
    )
    .await
    .expect("pipeline");

    let expected = reference_state();
    let got = final_state.lock().unwrap().clone();

    assert_eq!(got.character.as_deref(), Some("Xophina"));
    assert_eq!(got.level, expected.level);
    assert_eq!(got.level, 3);
    assert_eq!(got.act, expected.act);
    assert_eq!(got.act, Some(1));
    assert_eq!(got.deaths, expected.deaths);
    assert_eq!(got.deaths, 1);
    assert_eq!(
        got.zone.as_ref().map(|z| z.name.clone()),
        Some("The Coast".to_string())
    );
    assert_eq!(ctl.position(), ctl.total());
}

#[tokio::test]
async fn pause_and_step_release_exactly_one_line_each() {
    let fixture = write_fixture(LOG).await;
    let ctl = ReplayControl::new(SpeedMultiplier::X1);
    ctl.pause();
    let source = ReplaySource::new(fixture.path(), ctl.clone(), InstantSleeper);
    let mut stream = Box::new(source).into_stream();

    // Paused: nothing should be released within a short window.
    assert!(
        tokio::time::timeout(Duration::from_millis(60), stream.next())
            .await
            .is_err(),
        "no line should be released while paused"
    );

    // One step -> exactly one line.
    ctl.step();
    let first = tokio::time::timeout(Duration::from_millis(500), stream.next())
        .await
        .expect("step should release a line")
        .expect("stream item")
        .expect("ok item");
    assert!(first.raw.contains("LOG FILE OPENING"));

    // Still paused after the single step: nothing more.
    assert!(
        tokio::time::timeout(Duration::from_millis(60), stream.next())
            .await
            .is_err(),
        "only one line per step"
    );

    // Resume -> the remainder flows; drain to completion.
    ctl.resume();
    let mut count = 1;
    while (tokio::time::timeout(Duration::from_millis(500), stream.next()).await)
        .ok()
        .flatten()
        .is_some()
    {
        count += 1;
    }
    assert_eq!(count, LOG.lines().count());
}

#[tokio::test]
async fn stop_terminates_replay_early() {
    let fixture = write_fixture(LOG).await;
    let ctl = ReplayControl::new(SpeedMultiplier::X1);
    ctl.pause();
    let source = ReplaySource::new(fixture.path(), ctl.clone(), InstantSleeper);
    let mut stream = Box::new(source).into_stream();

    ctl.stop();
    assert!(
        tokio::time::timeout(Duration::from_millis(200), stream.next())
            .await
            .expect("should resolve, not hang")
            .is_none(),
        "stopped stream yields nothing and ends"
    );
}

#[tokio::test]
async fn recording_round_trips_to_identical_state() {
    // Record while replaying, then replay the recording - final states must match.
    let fixture = write_fixture(LOG).await;
    let rec_dir = tempfile::tempdir().expect("dir");
    let recorder = exile_core::recorder::Recorder::create(rec_dir.path(), "test")
        .await
        .expect("recorder");
    let recording_path = recorder.path().to_path_buf();

    let ctl = ReplayControl::new(SpeedMultiplier::X100);
    let source = ReplaySource::new(fixture.path(), ctl, InstantSleeper);
    let first = Arc::new(Mutex::new(CampaignState::default()));
    let sink = first.clone();
    run_pipeline(
        Box::new(source).into_stream(),
        CampaignMachine::new(zones()),
        Some(recorder),
        LocalePack::EN,
        move |s, _| *sink.lock().unwrap() = s.clone(),
    )
    .await
    .expect("first pipeline");

    // Replay the recording we just wrote.
    let ctl2 = ReplayControl::new(SpeedMultiplier::X100);
    let source2 = ReplaySource::new(&recording_path, ctl2, InstantSleeper);
    let second = Arc::new(Mutex::new(CampaignState::default()));
    let sink2 = second.clone();
    run_pipeline(
        Box::new(source2).into_stream(),
        CampaignMachine::new(zones()),
        None,
        LocalePack::EN,
        move |s, _| *sink2.lock().unwrap() = s.clone(),
    )
    .await
    .expect("second pipeline");

    let a = first.lock().unwrap();
    let b = second.lock().unwrap();
    assert_eq!(a.level, b.level);
    assert_eq!(a.act, b.act);
    assert_eq!(a.deaths, b.deaths);
    assert_eq!(
        a.zone.as_ref().map(|z| &z.name),
        b.zone.as_ref().map(|z| &z.name)
    );
    assert_eq!(b.zone.as_ref().unwrap().name, "The Coast");
}

#[tokio::test]
async fn live_tail_reads_appended_lines() {
    // Start from an empty file reading from the top (`from_start`): because streams are lazy, EOF is
    // only captured on first poll, so `from_start` + empty file is the deterministic way to assert
    // the poll -> stat -> read -> split -> yield loop picks up appended content.
    let file = tempfile::NamedTempFile::new().expect("temp");

    let tail = LiveTail::open(file.path())
        .from_start()
        .with_poll_interval(Duration::from_millis(15));
    let mut stream = Box::new(tail).into_stream();

    // Append two new complete lines after tailing starts.
    let mut handle = tokio::fs::OpenOptions::new()
        .append(true)
        .open(file.path())
        .await
        .expect("open append");
    handle
        .write_all(
            b"2018/05/13 16:05:41 2 9b0 [INFO Client 1012] : You have entered Lioneye's Watch.\r\n\
              2018/05/13 16:05:50 3 9b0 [INFO Client 1012] : Xophina (Witch) is now level 3\r\n",
        )
        .await
        .expect("append");
    handle.flush().await.expect("flush");

    let a = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("first line in time")
        .expect("item")
        .expect("ok");
    assert!(a.body.contains("Lioneye's Watch"), "got: {}", a.body);

    let b = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("second line in time")
        .expect("item")
        .expect("ok");
    assert!(b.body.contains("is now level 3"), "got: {}", b.body);
}

#[test]
fn history_primes_the_campaign_state() {
    use exile_core::pipeline::prime_from_history;

    let zones = Arc::new(
        ZoneTable::from_json_bytes(exile_data::AREAS_JSON, exile_data::AREAS_SCHEMA_VERSION)
            .unwrap(),
    );
    let mut machine = CampaignMachine::new(zones);
    let text = "ef] cut-off first line\n\
        2024/06/01 20:00:06 1006233 9c8 [INFO Client 1234] Generating level 2 area \"1_1_2\" with seed 1\n\
        2024/06/01 20:00:07 1007188 9c8 [INFO Client 1234] : You have entered The Coast.\n";
    prime_from_history(&mut machine, text, &LocalePack::EN);
    let st = machine.snapshot();
    assert_eq!(st.area_id.as_deref(), Some("1_1_2"));
    assert_eq!(st.act, Some(1));
}
