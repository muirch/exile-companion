//! Records raw log lines to a session file so a campaign can be replayed later.
//!
//! Lines are written **verbatim** (with reconstructed `\r\n`), so a recording is itself a valid
//! `Client.txt` that replays through the identical parser — the round-trip is tested.

use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;

use crate::error::RecorderError;

/// A tee that appends raw log lines to a session recording.
pub struct Recorder {
    file: tokio::fs::File,
    path: PathBuf,
}

impl Recorder {
    /// Create `dir/session-<session_id>.log`, truncating any existing file.
    pub async fn create(dir: &Path, session_id: &str) -> Result<Self, RecorderError> {
        let path = dir.join(format!("session-{session_id}.log"));
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .await?;
        Ok(Self { file, path })
    }

    /// The path being written.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one raw line (the original, pre-parse text) followed by `\r\n`.
    pub async fn write_line(&mut self, raw: &str) -> Result<(), RecorderError> {
        self.file.write_all(raw.as_bytes()).await?;
        self.file.write_all(b"\r\n").await?;
        Ok(())
    }

    /// Flush buffered writes to disk.
    pub async fn flush(&mut self) -> Result<(), RecorderError> {
        self.file.flush().await?;
        Ok(())
    }
}
