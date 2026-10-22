//! Live tailer for `Client.txt`.
//!
//! Polling is the source of truth (the file never rotates, grows unbounded, and the game holds it
//! open - so we must re-`stat` and read from a tracked offset regardless of any fs-notify signal).
//! Each tick: stat the length; if it shrank, the file was truncated/recreated -> reset to 0; if it
//! grew, read the appended bytes (shared read), split complete `\r\n` lines, and yield decoded
//! [`RawLine`]s. On start we seek to EOF so history isn't replayed.

use std::io::SeekFrom;
use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::{EventSource, LineStream};

/// A polling tail of a log file.
pub struct LiveTail {
    path: PathBuf,
    poll_interval: Duration,
    start_at_eof: bool,
}

impl LiveTail {
    /// Tail `path`, starting at end-of-file (skip existing history), polling every 400 ms.
    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            poll_interval: Duration::from_millis(400),
            start_at_eof: true,
        }
    }

    /// Override the poll interval.
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Read from the beginning instead of EOF (useful for tests and replaying an existing file live).
    pub fn from_start(mut self) -> Self {
        self.start_at_eof = false;
        self
    }
}

impl EventSource for LiveTail {
    fn into_stream(self: Box<Self>) -> LineStream {
        let LiveTail {
            path,
            poll_interval,
            start_at_eof,
        } = *self;

        Box::pin(async_stream::try_stream! {
            let mut offset: u64 = if start_at_eof {
                tokio::fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0)
            } else {
                0
            };
            let mut pending: Vec<u8> = Vec::new();
            let mut ticker = tokio::time::interval(poll_interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

            loop {
                ticker.tick().await;

                let len = match tokio::fs::metadata(&path).await {
                    Ok(meta) => meta.len(),
                    // File not there yet (or was deleted): wait for it to (re)appear.
                    Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                        offset = 0;
                        pending.clear();
                        continue;
                    }
                    Err(e) => Err(e)?,
                };

                // Truncation / recreation: start over from the top.
                if len < offset {
                    offset = 0;
                    pending.clear();
                }

                if len > offset {
                    let mut file = tokio::fs::File::open(&path).await?;
                    file.seek(SeekFrom::Start(offset)).await?;
                    let mut buf = Vec::new();
                    file.read_to_end(&mut buf).await?;
                    offset += buf.len() as u64;
                    pending.extend_from_slice(&buf);

                    while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
                        let mut line_bytes: Vec<u8> = pending.drain(..=pos).collect();
                        while matches!(line_bytes.last(), Some(b'\n') | Some(b'\r')) {
                            line_bytes.pop();
                        }
                        let line = String::from_utf8_lossy(&line_bytes);
                        if let Some(raw) = crate::parser::split_line(&line) {
                            yield raw;
                        }
                    }
                }
            }
        })
    }
}
