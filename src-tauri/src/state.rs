//! Shared application state held by Tauri (`app.manage`).

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use poebo_core::data::ZoneTable;
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
    /// The campaign data pack, shared with every state machine.
    pub zones: Arc<ZoneTable>,
    /// The running source (guarded; commands mutate it).
    pub run: Mutex<RunState>,
    /// Latest campaign snapshot, so `get_state` can hydrate a late-mounting UI synchronously.
    pub state_tx: watch::Sender<CampaignState>,
    /// Whether the overlay is in interactive ("grabbed") mode.
    pub grabbed: AtomicBool,
}

impl AppState {
    pub fn new(zones: Arc<ZoneTable>) -> Self {
        let (state_tx, _rx) = watch::channel(CampaignState::default());
        Self {
            zones,
            run: Mutex::new(RunState::default()),
            state_tx,
            grabbed: AtomicBool::new(false),
        }
    }

    /// A clone of the latest campaign state.
    pub fn current_state(&self) -> CampaignState {
        self.state_tx.borrow().clone()
    }
}
