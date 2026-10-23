//! Shared application state held by Tauri (`app.manage`).

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use exile_core::build::BuildState;
use exile_core::data::DataPack;
use exile_core::gems::GemTable;
use exile_core::route::{Route, RouteTracker};
use exile_core::source::ReplayControl;
use exile_core::state::CampaignState;
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
    pub gems: Arc<GemTable>,
    /// The running source (guarded, commands mutate it).
    pub run: Mutex<RunState>,
    /// Latest campaign snapshot, so `get_state` can hydrate a late-mounting UI synchronously.
    pub state_tx: watch::Sender<CampaignState>,
    /// The currently-imported Path of Building build (guarded, import commands mutate it).
    pub build: Mutex<BuildState>,
    /// Whether the overlay is in interactive ("grabbed") mode.
    pub grabbed: AtomicBool,
    /// The game, or one of our own windows, is in front.
    pub game_focused: AtomicBool,
    /// The game itself is in front.
    pub game_foreground: AtomicBool,
    pub route: Arc<Route>,
    pub anchor: Mutex<(f64, f64)>,
    pub tracker: Mutex<RouteTracker>,
    pub route_enabled: AtomicBool,
}

impl AppState {
    pub fn new(
        data: Arc<DataPack>,
        gems: Arc<GemTable>,
        route: Arc<Route>,
        route_edge: usize,
        route_enabled: bool,
    ) -> Self {
        let (state_tx, _rx) = watch::channel(CampaignState::default());
        Self {
            data,
            gems,
            run: Mutex::new(RunState::default()),
            state_tx,
            build: Mutex::new(BuildState::new()),
            // Global shortcuts don't fire on Wayland, so off Windows the controls stay reachable.
            grabbed: AtomicBool::new(!cfg!(windows)),
            game_focused: AtomicBool::new(!cfg!(windows)),
            game_foreground: AtomicBool::new(false),
            route,
            anchor: Mutex::new(crate::overlay::DEFAULT_ANCHOR),
            tracker: Mutex::new(RouteTracker::at(route_edge)),
            route_enabled: AtomicBool::new(route_enabled),
        }
    }

    /// A clone of the latest campaign state.
    pub fn current_state(&self) -> CampaignState {
        self.state_tx.borrow().clone()
    }
}
