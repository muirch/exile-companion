//! `exile-core` - pure, Tauri-free campaign logic for the PoE1 leveling assistant.
//!
//! # Design
//!
//! The game log, replay files, and (later) Path of Building inputs all become a stream of decoded
//! lines that one source-agnostic pipeline consumes (`parse -> campaign state -> callback`). Live
//! tailing and replay are two producers of the same stream, so they are behaviorally identical.
//!
//! This crate never depends on Tauri, so it is fully unit-testable headless:
//! `cargo test -p exile-core` runs anywhere with no GUI dependencies.
//!
//! # Error handling
//!
//! No `unwrap`/`expect` on production paths. Each subsystem returns a focused [`thiserror`] enum
//! from [`error`].

#![forbid(unsafe_code)]

pub mod agents;
pub mod build;
pub mod build_history;
pub mod clock;
pub mod data;
pub mod error;
pub mod events;
pub mod followed_guide;
pub mod game_window;
pub mod gemplan;
pub mod gems;
pub mod guidance;
pub mod hold;
pub mod locale;
pub mod logfile;
pub mod migrate;
pub mod parser;
pub mod pipeline;
pub mod pob_sync;
pub mod recorder;
pub mod resolve;
pub mod route;
pub mod source;
pub mod state;

/// Crate version, surfaced in diagnostics / the UI "about" panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
