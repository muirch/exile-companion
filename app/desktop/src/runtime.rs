//! Wires a `exile_core` source into a running pipeline whose updates are emitted to the frontend.
//!
//! Live and replay both spawn `run_pipeline` on Tauri's async runtime. The pipeline's `on_update`
//! emits `campaign://state-updated` and pushes into the state watch channel. Replay additionally
//! runs a small poller that emits `replay://progress`. Stopping aborts the task(s) and, for replay,
//! cancels via `ReplayControl`.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use exile_core::build::BuildSummary;
use exile_core::clock::TokioSleeper;
use exile_core::data::DataPack;
use exile_core::error::DataError;
use exile_core::gems::GemTable;
use exile_core::guidance::GuidanceEngine;
use exile_core::locale::LocalePack;
use exile_core::pipeline::{prime_from_history, run_pipeline};
use exile_core::route::{parse_route, Route, RouteTracker, RouteView};
use exile_core::source::{
    EventSource, LineStream, LiveTail, ReplayControl, ReplaySource, SpeedMultiplier,
};
use exile_core::state::{CampaignMachine, CampaignState};
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
        exile_data::AREAS_JSON,
        exile_data::AREAS_SCHEMA_VERSION,
        exile_data::QUESTS_JSON,
        exile_data::QUESTS_SCHEMA_VERSION,
        exile_data::TRIALS_JSON,
        exile_data::TRIALS_SCHEMA_VERSION,
    )?;
    Ok(Arc::new(pack))
}

pub fn load_route(data: &DataPack, league_start: bool) -> Arc<Route> {
    let mut defines = HashSet::new();
    if league_start {
        defines.insert("LEAGUE_START".to_string());
    }
    let source = exile_data::ROUTE_ACTS.join("\n");
    let (route, warnings) = parse_route(&source, &data.zones, &data.quests, &defines);
    for w in warnings {
        log::warn!("route line {}: {}", w.line, w.message);
    }
    Arc::new(route)
}

pub fn current_route_view(app: &AppHandle) -> Option<RouteView> {
    let app_state = app.state::<AppState>();
    let edge = lock(&app_state.tracker).active();
    let mut view = app_state.route.view(edge)?;
    let builds = lock(&app_state.build);
    if let Some(build) = builds.build() {
        if let Some(class) = build.class_name.as_deref() {
            let saved = settings::load(app).unwrap_or_default();
            let plan = exile_core::gemplan::plan_with_stages(
                build,
                class,
                &app_state.gems,
                &saved.preset_stages,
            );
            exile_core::gemplan::annotate(&mut view, &plan, saved.gem_hints);
        }
    }
    Some(view)
}

pub fn load_gems() -> Arc<GemTable> {
    match GemTable::from_json_bytes(exile_data::GEMS_JSON, exile_data::GEM_SOURCES_JSON) {
        Ok(t) => Arc::new(t),
        Err(e) => {
            log::error!("gem data: {e}");
            Arc::new(GemTable::default())
        }
    }
}

pub fn emit_route(app: &AppHandle) {
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
pub fn emit_build_summary(app: &AppHandle) {
    let _ = app.emit("build://updated", &current_build_summary(app));
}

fn build_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("build.xml"))
}

/// Loads the build saved by the last import, so gem steps survive a restart.
pub fn restore_build(app: &AppHandle) {
    let Some(path) = build_path(app).filter(|p| p.exists()) else {
        return;
    };
    match std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|xml| exile_pob::parse_xml(&xml).map_err(|e| e.to_string()))
    {
        Ok(build) => lock(&app.state::<AppState>().build).set(build),
        Err(e) => log::warn!("saved build {}: {e}", path.display()),
    }
}

/// The assistant changes build.xml from its own process. Picks those changes up while the app runs.
pub fn watch_build(app: AppHandle) {
    let stamp = |app: &AppHandle| {
        let meta = std::fs::metadata(build_path(app)?).ok()?;
        Some((meta.modified().ok()?, meta.len()))
    };
    tauri::async_runtime::spawn(async move {
        let mut last = stamp(&app);
        let mut sync_error = None;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            sync_pob(&app, &mut sync_error);
            let now = stamp(&app);
            if now != last && now.is_some() {
                reload_build(&app);
                emit_build_summary(&app);
                emit_guidance(&app, &app.state::<AppState>().current_state());
                emit_route(&app);
            }
            last = now;
        }
    });
}

/// Keeps the build and its file in PoB's Builds folder the same when the player turned that on.
/// A pull rewrites build.xml, and watch_build picks that up like any other change.
pub fn sync_pob(app: &AppHandle, last_error: &mut Option<String>) {
    let s = settings::load(app).unwrap_or_default();
    if !s.pob_sync {
        return;
    }
    let (Some(build), Some(file)) = (build_path(app), crate::pob::sync_file(app, &s)) else {
        return;
    };
    match exile_core::pob_sync::sync(&build, &file) {
        Ok(synced) => {
            if synced != exile_core::pob_sync::Synced::Nothing {
                log::info!("PoB sync with {}: {synced:?}", file.display());
            }
            *last_error = None;
        }
        Err(e) => {
            let e = format!("PoB sync with {}: {e}", file.display());
            if last_error.as_ref() != Some(&e) {
                log::warn!("{e}");
            }
            *last_error = Some(e);
        }
    }
}

/// The build file changed outside an import. Stages set for the old build's presets go when the
/// presets aren't the same any more, as on an import.
fn reload_build(app: &AppHandle) {
    let Some(path) = build_path(app) else {
        return;
    };
    let build = match std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|xml| exile_pob::parse_xml(&xml).map_err(|e| e.to_string()))
    {
        Ok(build) => build,
        Err(e) => {
            log::warn!("changed build {}: {e}", path.display());
            return;
        }
    };
    let app_state = app.state::<AppState>();
    let mut builds = lock(&app_state.build);
    let same = builds
        .build()
        .is_some_and(|old| exile_core::gemplan::same_presets(old, &build));
    if !same {
        if let Ok(mut s) = settings::load(app) {
            if !s.preset_stages.is_empty() {
                s.preset_stages.clear();
                let _ = settings::save(app, &s);
            }
        }
    }
    builds.set(build);
}

/// Parse and store a freshly-imported build, save it for the next start, then push the new build
/// summary, guidance and route.
pub fn import_build(app: &AppHandle, xml: Vec<u8>) -> Result<BuildSummary, String> {
    let build = exile_pob::parse_xml(&xml).map_err(|e| {
        log::warn!("build import: {e}");
        "That's not a Path of Building build.".to_string()
    })?;
    // Stages belong to the old build's skill sets.
    if let Ok(mut s) = settings::load(app) {
        s.preset_stages.clear();
        let _ = settings::save(app, &s);
    }
    if let Some(path) = build_path(app) {
        let saved = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, &xml));
        if let Err(e) = saved {
            log::warn!("could not save the build to {}: {e}", path.display());
        }
    }
    let app_state = app.state::<AppState>();
    lock(&app_state.build).set(build);
    let summary = current_build_summary(app);
    emit_build_summary(app);
    emit_guidance(app, &app_state.current_state());
    emit_route(app);
    summary.ok_or_else(|| "imported build produced no summary".to_string())
}

/// Forget the loaded build and push the cleared state.
pub fn clear_build(app: &AppHandle) {
    // Otherwise the next sync brings the build straight back from PoB.
    if let Ok(mut s) = settings::load(app) {
        if s.pob_sync {
            s.pob_sync = false;
            let _ = settings::save(app, &s);
        }
    }
    if let Some(path) = build_path(app) {
        let _ = std::fs::remove_file(path);
    }
    let app_state = app.state::<AppState>();
    if let Ok(mut bs) = app_state.build.lock() {
        bs.clear();
    }
    emit_build_summary(app);
    emit_guidance(app, &app_state.current_state());
    emit_route(app);
}

/// The current build summary, if a build is loaded.
pub fn current_build_summary(app: &AppHandle) -> Option<BuildSummary> {
    let app_state = app.state::<AppState>();
    let builds = lock(&app_state.build);
    let build = builds.build()?;
    let mut summary = BuildSummary::from_build(build);
    if let Some(class) = build.class_name.as_deref() {
        let stages = settings::load(app).unwrap_or_default().preset_stages;
        let plan = exile_core::gemplan::plan_with_stages(build, class, &app_state.gems, &stages);
        summary.not_in_campaign = plan.not_in_campaign;
        summary.mules = plan.mules;
    }
    Some(summary)
}

/// The next objective for the current state + loaded build (for on-demand hydration).
pub fn current_guidance(app: &AppHandle) -> exile_core::guidance::NextObjective {
    let app_state = app.state::<AppState>();
    let state = app_state.current_state();
    let objective = GuidanceEngine::next(&state, &lock(&app_state.build), &app_state.data);
    objective
}

fn read_log_tail(path: &str) -> String {
    exile_core::logfile::read_tail(
        std::path::Path::new(path),
        exile_core::logfile::HISTORY_BYTES,
    )
    .unwrap_or_default()
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
            tx.send_replace(st.clone());
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
    state.state_tx.send_replace(primed.clone());
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
