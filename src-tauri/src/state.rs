//! Shared application state held by Tauri (`app.manage`).

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use poebo_core::build::BuildState;
use poebo_core::data::DataPack;
use poebo_core::route::{Route, RouteTracker};
use poebo_core::source::ReplayControl;
use poebo_core::state::CampaignState;
use tauri::async_runtime::JoinHandle;
use tokio::sync::watch;

/// Which kind of source is currently running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceMode {
    Idle,
    Live,
    Replay,
}

/// The currently-running source task and its controls.
pub struct RunState {
    pub mode: SourceMode,
    pub task: Option<JoinHandle<()>>,
    pub progress_task: Option<JoinHandle<()>>,
    pub replay: Option<Arc<ReplayControl>>,
    pub log_path: Option<String>,
}

impl Default for RunState {
    fn default() -> Self {
        Self {
            mode: SourceMode::Idle,
            task: None,
            progress_task: None,
            replay: None,
            log_path: None,
        }
    }
}

/// Managed application state.
pub struct AppState {
    /// The campaign knowledge base (zones, quests, trials), shared read-only.
    pub data: Arc<DataPack>,
    /// The running source (guarded; commands mutate it).
    pub run: Mutex<RunState>,
    /// Latest campaign snapshot, so `get_state` can hydrate a late-mounting UI synchronously.
    pub state_tx: watch::Sender<CampaignState>,
    /// The currently-imported Path of Building build (guarded; import commands mutate it).
    pub build: Mutex<BuildState>,
    /// Whether the overlay is in interactive ("grabbed") mode.
    pub grabbed: AtomicBool,
    pub route: Arc<Route>,
    pub tracker: Mutex<RouteTracker>,
}

impl AppState {
    pub fn new(data: Arc<DataPack>, route: Arc<Route>, route_edge: usize) -> Self {
        let (state_tx, _rx) = watch::channel(CampaignState::default());
        Self {
            data,
            run: Mutex::new(RunState::default()),
            state_tx,
            build: Mutex::new(BuildState::new()),
            // Start interactive so the controls are reachable without a global hotkey
            // (global shortcuts do not fire on Wayland).
            grabbed: AtomicBool::new(true),
            route,
            tracker: Mutex::new(RouteTracker::at(route_edge)),
        }
    }

    /// A clone of the latest campaign state.
    pub fn current_state(&self) -> CampaignState {
        self.state_tx.borrow().clone()
    }
}
