//! Wires a `poebo_core` source into a running pipeline whose updates are emitted to the frontend.
//!
//! Live and replay both spawn `run_pipeline` on Tauri's async runtime; the pipeline's `on_update`
//! emits `campaign://state-updated` and pushes into the state watch channel. Replay additionally
//! runs a small poller that emits `replay://progress`. Stopping aborts the task(s) and, for replay,
//! cancels via `ReplayControl`.

use std::sync::Arc;
use std::time::Duration;

use poebo_core::build::BuildSummary;
use poebo_core::clock::TokioSleeper;
use poebo_core::data::DataPack;
use poebo_core::error::DataError;
use poebo_core::guidance::GuidanceEngine;
use poebo_core::locale::LocalePack;
use poebo_core::pipeline::run_pipeline;
use poebo_core::source::{
    EventSource, LineStream, LiveTail, ReplayControl, ReplaySource, SpeedMultiplier,
};
use poebo_core::state::{CampaignMachine, CampaignState};
use poebo_pob::PobBuild;
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::{AppState, SourceMode};

/// Load and index the embedded campaign data pack (zones + quests + trials).
pub fn load_data_pack() -> Result<Arc<DataPack>, DataError> {
    let pack = DataPack::from_bytes(
        poebo_data::AREAS_JSON,
        poebo_data::AREAS_SCHEMA_VERSION,
        poebo_data::QUESTS_JSON,
        poebo_data::QUESTS_SCHEMA_VERSION,
        poebo_data::TRIALS_JSON,
        poebo_data::TRIALS_SCHEMA_VERSION,
    )?;
    Ok(Arc::new(pack))
}

/// Recompute and emit the next objective for a given campaign state, using the loaded build.
fn emit_guidance(app: &AppHandle, state: &CampaignState) {
    let app_state = app.state::<AppState>();
    let objective = {
        let build = match app_state.build.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        GuidanceEngine::next(state, &build, &app_state.data)
    };
    let _ = app.emit("guidance://next", &objective);
}

/// Emit the current build summary (`None` when no build is loaded).
fn emit_build_summary(app: &AppHandle) {
    let app_state = app.state::<AppState>();
    let summary = match app_state.build.lock() {
        Ok(guard) => guard.summary(),
        Err(poisoned) => poisoned.into_inner().summary(),
    };
    let _ = app.emit("build://updated", &summary);
}

/// Store a freshly-imported build, then push the new build summary and recomputed guidance.
pub fn import_build(app: &AppHandle, build: PobBuild) -> Result<BuildSummary, String> {
    let app_state = app.state::<AppState>();
    let summary = {
        let mut bs = app_state.build.lock().map_err(|e| e.to_string())?;
        bs.set(build);
        bs.summary()
    };
    emit_build_summary(app);
    emit_guidance(app, &app_state.current_state());
    summary.ok_or_else(|| "imported build produced no summary".to_string())
}

/// Forget the loaded build and push the cleared state.
pub fn clear_build(app: &AppHandle) {
    let app_state = app.state::<AppState>();
    if let Ok(mut bs) = app_state.build.lock() {
        bs.clear();
    }
    emit_build_summary(app);
    emit_guidance(app, &app_state.current_state());
}

/// The current build summary, if a build is loaded.
pub fn current_build_summary(app: &AppHandle) -> Option<BuildSummary> {
    let app_state = app.state::<AppState>();
    match app_state.build.lock() {
        Ok(guard) => guard.summary(),
        Err(poisoned) => poisoned.into_inner().summary(),
    }
}

/// The next objective for the current state + loaded build (for on-demand hydration).
pub fn current_guidance(app: &AppHandle) -> poebo_core::guidance::NextObjective {
    let app_state = app.state::<AppState>();
    let state = app_state.current_state();
    let build = match app_state.build.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    GuidanceEngine::next(&state, &build, &app_state.data)
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

/// Spawn the parse -> state pipeline, emitting updates to the frontend.
fn spawn_pipeline(app: AppHandle, stream: LineStream, machine: CampaignMachine) -> JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let tx = app.state::<AppState>().state_tx.clone();
        let cb_app = app.clone();
        let result = run_pipeline(stream, machine, None, LocalePack::EN, move |st, _delta| {
            let _ = cb_app.emit("campaign://state-updated", st);
            let _ = tx.send(st.clone());
            // Guidance depends on the campaign state, so recompute it on every meaningful change.
            emit_guidance(&cb_app, st);
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
    let machine = CampaignMachine::new(Arc::new(state.data.zones.clone()));
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
    let machine = CampaignMachine::new(Arc::new(state.data.zones.clone()));
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
