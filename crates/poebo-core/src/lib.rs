//! `poebo-core` — pure, Tauri-free campaign logic for the PoE1 leveling assistant.
//!
//! # Design
//!
//! The game log, replay files, and (later) Path of Building inputs all become a stream of decoded
//! lines that one source-agnostic pipeline consumes (`parse → campaign state → callback`). Live
//! tailing and replay are two producers of the same stream, so they are behaviorally identical.
//!
//! This crate never depends on Tauri, so it is fully unit-testable headless:
//! `cargo test -p poebo-core` runs anywhere with no GUI dependencies.
//!
//! # Error handling
//!
//! No `unwrap`/`expect` on production paths. Each subsystem returns a focused [`thiserror`] enum
//! from [`error`].

#![forbid(unsafe_code)]

pub mod data;
pub mod error;
pub mod events;
pub mod locale;
pub mod parser;
// Added in later build-order steps:
//   pub mod state;     // step 4
//   pub mod clock;     // step 4
//   pub mod source;    // step 5
//   pub mod recorder;  // step 5
//   pub mod pipeline;  // step 5

/// Crate version, surfaced in diagnostics / the UI "about" panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
