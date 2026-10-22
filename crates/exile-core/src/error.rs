//! Error types for `exile-core`. Each subsystem gets a focused [`thiserror`] enum so callers can
//! match on precise failure modes. No `unwrap`/`expect` on production paths.

use std::path::PathBuf;

/// Errors from loading or validating the campaign data pack (`data-pack/areas.json`).
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    /// The JSON did not deserialize into the expected data-pack shape.
    #[error("failed to parse data pack JSON: {0}")]
    Parse(#[from] serde_json::Error),

    /// The pack's `schema_version` does not match what this build understands.
    #[error("data pack schema version {found} is unsupported (this build expects {expected})")]
    SchemaMismatch { found: u32, expected: u32 },

    /// Reading the pack from disk failed (used by the file-based loader; the embedded loader
    /// takes bytes directly).
    #[error("failed to read data pack file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Errors from a log line source (live tail or replay) - surfaced as stream items.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("log source I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors writing a recording (tee of raw log lines).
#[derive(Debug, thiserror::Error)]
pub enum RecorderError {
    #[error("recorder I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors from running the parse -> state pipeline over a source.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error(transparent)]
    Source(#[from] SourceError),
    #[error(transparent)]
    Recorder(#[from] RecorderError),
}
