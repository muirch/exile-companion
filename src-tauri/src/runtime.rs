//! Wires a `poebo_core` source into a running pipeline whose updates are emitted to the frontend.
//!
//! Live and replay both spawn `run_pipeline` on Tauri's async runtime; the pipeline's `on_update`
//! emits `campaign://state-updated` and pushes into the state watch channel. Replay additionally
//! runs a small poller that emits `replay://progress`. Stopping aborts the task(s) and, for replay,
//! cancels via `ReplayControl`.

use std::sync::Arc;
use std::time::Duration;

use poebo_core::clock::TokioSleeper;
use poebo_core::data::ZoneTable;
use poebo_core::error::DataError;
use poebo_core::locale::LocalePack;
use poebo_core::pipeline::run_pipeline;
use poebo_core::source::{
    EventSource, LineStream, LiveTail, ReplayControl, ReplaySource, SpeedMultiplier,
};
use poebo_core::state::{CampaignMachine, CampaignState};
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::{AppState, SourceMode};

/// Load and index the embedded campaign data pack.
pub fn load_zone_table() -> Result<Arc<ZoneTable>, DataError> {
    let table =
        ZoneTable::from_json_bytes(poebo_data::AREAS_JSON, poebo_data::PACK_SCHEMA_VERSION)?;
    Ok(Arc::new(table))
}

#[derive(Serialize, Clone)]
struct SourceStatus {
    mode: SourceMode,
    path: Option<String>,
    error: Option<String>,
}

#[derive(Serialize, Clone)]
struct ReplayProgress {
    position: usize,
    total: usize,
    speed: SpeedMultiplier,
    paused: bool,
}

/// Stop and clear whatever source is currently running.
pub fn stop_current(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut run = match state.run.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(ctl) = run.replay.take() {
        ctl.stop();
    }
    if let Some(task) = run.progress_task.take() {
        task.abort();
    }
    if let Some(task) = run.task.take() {
        task.abort();
    }
    run.mode = SourceMode::Idle;
    run.log_path = None;
}

/// Spawn the parse → state pipeline, emitting updates to the frontend.
fn spawn_pipeline(app: AppHandle, stream: LineStream, machine: CampaignMachine) -> JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let tx = app.state::<AppState>().state_tx.clone();
        let cb_app = app.clone();
        let result = run_pipeline(stream, machine, None, LocalePack::EN, move |st, _delta| {
            let _ = cb_app.emit("campaign://state-updated", st);
            let _ = tx.send(st.clone());
        })
        .await;

        let error = result.err().map(|e| e.to_string());
        let _ = app.emit(
            "source://status",
            SourceStatus {
                mode: SourceMode::Idle,
                path: None,
                error,
            },
        );
    })
}

/// Poll replay progress and emit it until the replay finishes.
fn spawn_progress(app: AppHandle, ctl: Arc<ReplayControl>) -> JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        loop {
            let total = ctl.total();
            let position = ctl.position();
            let _ = app.emit(
                "replay://progress",
                ReplayProgress {
                    position,
                    total,
                    speed: SpeedMultiplier::from_factor(ctl.speed()),
                    paused: ctl.is_paused(),
                },
            );
            if total > 0 && position >= total {
                break;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    })
}

/// Start tailing a live `Client.txt`.
pub fn start_live(app: AppHandle, path: String) -> Result<CampaignState, String> {
    stop_current(&app);
    let state = app.state::<AppState>();
    let machine = CampaignMachine::new(state.zones.clone());
    let stream = Box::new(LiveTail::open(&path)).into_stream();
    let task = spawn_pipeline(app.clone(), stream, machine);

    {
        let mut run = state.run.lock().map_err(|e| e.to_string())?;
        run.mode = SourceMode::Live;
        run.task = Some(task);
        run.log_path = Some(path.clone());
    }
    let _ = app.emit(
        "source://status",
        SourceStatus {
            mode: SourceMode::Live,
            path: Some(path),
            error: None,
        },
    );
    Ok(state.current_state())
}

/// Start replaying a recorded log.
pub fn start_replay(app: AppHandle, path: String, speed: SpeedMultiplier) -> Result<(), String> {
    stop_current(&app);
    let state = app.state::<AppState>();
    let ctl = ReplayControl::new(speed);
    let machine = CampaignMachine::new(state.zones.clone());
    let stream = Box::new(ReplaySource::new(&path, ctl.clone(), TokioSleeper)).into_stream();
    let task = spawn_pipeline(app.clone(), stream, machine);
    let progress_task = spawn_progress(app.clone(), ctl.clone());

    {
        let mut run = state.run.lock().map_err(|e| e.to_string())?;
        run.mode = SourceMode::Replay;
        run.task = Some(task);
        run.progress_task = Some(progress_task);
        run.replay = Some(ctl);
        run.log_path = Some(path.clone());
    }
    let _ = app.emit(
        "source://status",
        SourceStatus {
            mode: SourceMode::Replay,
            path: Some(path),
            error: None,
        },
    );
    Ok(())
}

/// Apply a mutation to the active replay control, if any.
fn with_replay(app: &AppHandle, f: impl FnOnce(&ReplayControl)) {
    let state = app.state::<AppState>();
    let run = match state.run.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(ctl) = run.replay.as_deref() {
        f(ctl);
    }
}

pub fn replay_pause(app: &AppHandle) {
    with_replay(app, ReplayControl::pause);
}

pub fn replay_resume(app: &AppHandle) {
    with_replay(app, ReplayControl::resume);
}

pub fn replay_step(app: &AppHandle) {
    with_replay(app, ReplayControl::step);
}

pub fn replay_set_speed(app: &AppHandle, speed: SpeedMultiplier) {
    with_replay(app, |ctl| ctl.set_speed(speed));
}
