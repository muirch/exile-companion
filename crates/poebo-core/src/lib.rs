//! `poebo-core` - pure, Tauri-free campaign logic for the PoE1 leveling assistant.
//!
//! # Design
//!
//! The game log, replay files, and (later) Path of Building inputs all become a stream of decoded
//! lines that one source-agnostic pipeline consumes (`parse -> campaign state -> callback`). Live
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

pub mod build;
pub mod clock;
pub mod data;
pub mod error;
pub mod events;
pub mod guidance;
pub mod locale;
pub mod parser;
pub mod pipeline;
pub mod recorder;
pub mod resolve;
pub mod source;
pub mod state;

/// Crate version, surfaced in diagnostics / the UI "about" panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
