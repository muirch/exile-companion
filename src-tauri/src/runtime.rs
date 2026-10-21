//! Wires a `poebo_core` source into a running pipeline whose updates are emitted to the frontend.
//!
//! Live and replay both spawn `run_pipeline` on Tauri's async runtime; the pipeline's `on_update`
//! emits `campaign://state-updated` and pushes into the state watch channel. Replay additionally
//! runs a small poller that emits `replay://progress`. Stopping aborts the task(s) and, for replay,
//! cancels via `ReplayControl`.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use poebo_core::build::BuildSummary;
use poebo_core::clock::TokioSleeper;
use poebo_core::data::DataPack;
use poebo_core::error::DataError;
use poebo_core::guidance::GuidanceEngine;
use poebo_core::locale::LocalePack;
use poebo_core::pipeline::{prime_from_history, run_pipeline};
use poebo_core::route::{parse_route, Route, RouteTracker, RouteView};
use poebo_core::source::{
    EventSource, LineStream, LiveTail, ReplayControl, ReplaySource, SpeedMultiplier,
};
use poebo_core::state::{CampaignMachine, CampaignState};
use poebo_pob::PobBuild;
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager};

use crate::settings;
use crate::state::{AppState, RunState, SourceMode};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

pub fn load_route(data: &DataPack, league_start: bool) -> Arc<Route> {
    let mut defines = HashSet::new();
    if league_start {
        defines.insert("LEAGUE_START".to_string());
    }
    let source = poebo_data::ROUTE_ACTS.join("\n");
    let (route, warnings) = parse_route(&source, &data.zones, &data.quests, &defines);
    for w in warnings {
        log::warn!("route line {}: {}", w.line, w.message);
    }
    Arc::new(route)
}

pub fn current_route_view(app: &AppHandle) -> Option<RouteView> {
    let app_state = app.state::<AppState>();
    let edge = lock(&app_state.tracker).active();
    app_state.route.view(edge)
}

fn emit_route(app: &AppHandle) {
    let _ = app.emit("route://view", current_route_view(app));
}

// Replays start from the first edge and must not overwrite the player's saved place.
fn save_route_edge(app: &AppHandle) {
    let app_state = app.state::<AppState>();
    if lock(&app_state.run).mode == SourceMode::Replay {
        return;
    }
    let edge = lock(&app_state.tracker).active();
    if let Ok(mut s) = settings::load(app) {
        s.route_edge = edge;
        let _ = settings::save(app, &s);
    }
}

pub enum RouteMove {
    Next,
    Prev,
    Reset,
}

pub fn move_route(app: &AppHandle, mv: RouteMove) -> Option<RouteView> {
    {
        let app_state = app.state::<AppState>();
        let mut tracker = lock(&app_state.tracker);
        match mv {
            RouteMove::Next => tracker.next(&app_state.route),
            RouteMove::Prev => tracker.prev(),
            RouteMove::Reset => tracker.reset(),
        }
    }
    save_route_edge(app);
    emit_route(app);
    current_route_view(app)
}

fn follow_route(app: &AppHandle, area_id: &str, level: u32) {
    let advanced = {
        let app_state = app.state::<AppState>();
        let mut tracker = lock(&app_state.tracker);
        tracker.on_area(&app_state.route, area_id, (level > 0).then_some(level))
    };
    if advanced {
        save_route_edge(app);
        emit_route(app);
    }
}

/// Recompute and emit the next objective for a given campaign state, using the loaded build.
fn emit_guidance(app: &AppHandle, state: &CampaignState) {
    let app_state = app.state::<AppState>();
    let objective = GuidanceEngine::next(state, &lock(&app_state.build), &app_state.data);
    let _ = app.emit("guidance://next", &objective);
}

/// Emit the current build summary (`None` when no build is loaded).
fn emit_build_summary(app: &AppHandle) {
    let app_state = app.state::<AppState>();
    let summary = lock(&app_state.build).summary();
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
    let summary = lock(&app_state.build).summary();
    summary
}

/// The next objective for the current state + loaded build (for on-demand hydration).
pub fn current_guidance(app: &AppHandle) -> poebo_core::guidance::NextObjective {
    let app_state = app.state::<AppState>();
    let state = app_state.current_state();
    let objective = GuidanceEngine::next(&state, &lock(&app_state.build), &app_state.data);
    objective
}

// Enough for the last few zones of a session; Client.txt itself can be hundreds of MB.
const HISTORY_BYTES: u64 = 512 * 1024;

fn read_log_tail(path: &str) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut buf = Vec::new();
    if file
        .seek(SeekFrom::Start(len.saturating_sub(HISTORY_BYTES)))
        .is_ok()
    {
        let _ = file.read_to_end(&mut buf);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

pub fn current_source(app: &AppHandle) -> SourceStatus {
    let app_state = app.state::<AppState>();
    let run = lock(&app_state.run);
    SourceStatus {
        mode: run.mode,
        path: run.log_path.clone(),
        error: None,
    }
}

#[derive(Serialize, Clone)]
pub struct SourceStatus {
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
    let was_replay = {
        let mut run = lock(&state.run);
        let was_replay = run.mode == SourceMode::Replay;
        stop_run(&mut run);
        was_replay
    };
    if was_replay {
        let saved_edge = settings::load(app).map(|s| s.route_edge).unwrap_or(0);
        *lock(&state.tracker) = RouteTracker::at(saved_edge);
        emit_route(app);
    }
}

fn stop_run(run: &mut RunState) {
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
        let result = run_pipeline(stream, machine, None, LocalePack::EN, move |st, delta| {
            let _ = cb_app.emit("campaign://state-updated", st);
            let _ = tx.send(st.clone());
            if delta.zone_changed {
                if let Some(area_id) = &st.area_id {
                    follow_route(&cb_app, area_id, st.level);
                }
            }
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
    let mut saved = settings::load(&app).unwrap_or_default();
    *lock(&state.tracker) = RouteTracker::at(saved.route_edge);
    saved.last_log_path = Some(path.clone());
    let _ = settings::save(&app, &saved);

    let mut machine = CampaignMachine::new(Arc::new(state.data.zones.clone()));
    prime_from_history(&mut machine, &read_log_tail(&path), &LocalePack::EN);
    let primed = machine.snapshot().clone();
    let _ = state.state_tx.send(primed.clone());
    let _ = app.emit("campaign://state-updated", &primed);
    if let Some(area_id) = &primed.area_id {
        let level = (primed.level > 0).then_some(primed.level);
        lock(&state.tracker).on_area(&state.route, area_id, level);
        save_route_edge(&app);
    }
    emit_route(&app);

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
    lock(&state.run).mode = SourceMode::Replay;
    lock(&state.tracker).reset();
    emit_route(&app);
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
