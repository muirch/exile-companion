//! Path of Building's own calculations, run headless. The `exile-pob-engine` binary embeds LuaJIT
//! and loads a PoB folder (an installed copy or a checkout). [`Engine`] starts it as a child
//! process and asks it questions about a build.

mod client;
mod host;
mod layout;
pub mod protocol;

use std::path::PathBuf;

pub use client::Engine;
pub use host::Host;
pub use layout::PobDir;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("{0} doesn't look like a Path of Building folder (no Launch.lua and lua/xml.lua)")]
    NotPob(PathBuf),
    #[error("couldn't start the engine: {0}")]
    Spawn(std::io::Error),
    #[error("{0}")]
    Io(std::io::Error),
    #[error("the engine stopped:\n{0}")]
    Exited(String),
    #[error("the engine took too long on {0} and was stopped")]
    Timeout(String),
    #[error("unexpected answer from the engine: {0}")]
    Protocol(serde_json::Error),
    #[error("no engine method {0}")]
    UnknownMethod(String),
    #[error("Path of Building: {0}")]
    Pob(String),
}
