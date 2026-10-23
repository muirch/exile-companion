//! Changes the assistant made to the player's build file, with the build as it was before each
//! one, so they can be listed and undone.
//!
//! The history belongs to one line of edits. When the file no longer matches the last change
//! (the player imported another build), the old entries no longer apply: they aren't listed, undo
//! refuses, and the next change starts a new history.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Entries kept. The oldest ones and their snapshots go first.
pub const LIMIT: usize = 50;

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("nothing to undo")]
    Empty,
    #[error("the build changed since the last change, so it can't be undone")]
    Changed,
    #[error("history I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("history file is broken: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    /// RFC 3339, UTC.
    pub time: String,
    pub reason: String,
    pub changes: Value,
    /// What the change did to the numbers, as the engine measured it.
    pub effect: Value,
    /// Hash of the build file right after this change.
    after: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Log {
    next_id: u64,
    entries: Vec<Entry>,
}

pub struct BuildHistory {
    build: PathBuf,
    dir: PathBuf,
}

impl BuildHistory {
    /// History for the build at `build`, kept next to it in `<stem>-history`.
    pub fn new(build: impl Into<PathBuf>) -> Self {
        let build = build.into();
        let stem = build
            .file_stem()
            .map_or("build".into(), |s| s.to_string_lossy());
        let dir = build.with_file_name(format!("{stem}-history"));
        Self { build, dir }
    }

    /// Changes in the order they were made, empty when the build file moved on without them.
    pub fn entries(&self) -> Result<Vec<Entry>, HistoryError> {
        let log = self.read()?;
        Ok(if self.current(&log)? {
            log.entries
        } else {
            Vec::new()
        })
    }

    /// Writes `after` as the build and records the change. `before` is the build the change was
    /// made on.
    pub fn record(
        &self,
        before: &[u8],
        after: &[u8],
        reason: &str,
        changes: Value,
        effect: Value,
    ) -> Result<Entry, HistoryError> {
        let mut log = self.read()?;
        if log
            .entries
            .last()
            .is_some_and(|last| last.after != hash(before))
        {
            for e in log.entries.drain(..) {
                remove(&self.snapshot(e.id))?;
            }
        }
        let entry = Entry {
            id: log.next_id,
            time: now(),
            reason: reason.to_string(),
            changes,
            effect,
            after: hash(after),
        };
        log.next_id += 1;
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(self.snapshot(entry.id), before)?;
        write_atomic(&self.build, after)?;
        log.entries.push(entry.clone());
        while log.entries.len() > LIMIT {
            let old = log.entries.remove(0);
            remove(&self.snapshot(old.id))?;
        }
        self.write(&log)?;
        Ok(entry)
    }

    /// Puts back the build as it was before the last change. Returns that change and the build.
    pub fn undo(&self) -> Result<(Entry, Vec<u8>), HistoryError> {
        let mut log = self.read()?;
        if log.entries.is_empty() {
            return Err(HistoryError::Empty);
        }
        if !self.current(&log)? {
            return Err(HistoryError::Changed);
        }
        let entry = log.entries.pop().ok_or(HistoryError::Empty)?;
        let before = std::fs::read(self.snapshot(entry.id))?;
        write_atomic(&self.build, &before)?;
        self.write(&log)?;
        remove(&self.snapshot(entry.id))?;
        Ok((entry, before))
    }

    fn current(&self, log: &Log) -> Result<bool, HistoryError> {
        let Some(last) = log.entries.last() else {
            return Ok(true);
        };
        match std::fs::read(&self.build) {
            Ok(build) => Ok(hash(&build) == last.after),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    fn snapshot(&self, id: u64) -> PathBuf {
        self.dir.join(format!("{id}.xml"))
    }

    fn read(&self) -> Result<Log, HistoryError> {
        match std::fs::read(self.dir.join("history.json")) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Log::default()),
            Err(e) => Err(e.into()),
        }
    }

    fn write(&self, log: &Log) -> Result<(), HistoryError> {
        std::fs::create_dir_all(&self.dir)?;
        write_atomic(
            &self.dir.join("history.json"),
            &serde_json::to_vec_pretty(log)?,
        )?;
        Ok(())
    }
}

/// The app reads the build file while we write it, so it must never see half a file.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

fn remove(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// FNV-1a. Stored in the history file, so it must not change between Rust versions the way
/// std's DefaultHasher may.
fn hash(bytes: &[u8]) -> String {
    let h = bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    format!("{h:016x}")
}

fn now() -> String {
    time::OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .ok()
        .and_then(|t| {
            t.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn setup() -> (tempfile::TempDir, BuildHistory) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("build.xml");
        std::fs::write(&path, "v0").unwrap();
        (dir, BuildHistory::new(path))
    }

    fn build(dir: &tempfile::TempDir) -> String {
        std::fs::read_to_string(dir.path().join("build.xml")).unwrap()
    }

    #[test]
    fn records_and_undoes_in_order() {
        let (dir, h) = setup();
        h.record(b"v0", b"v1", "first", json!([1]), json!({}))
            .unwrap();
        h.record(b"v1", b"v2", "second", json!([2]), json!({}))
            .unwrap();
        assert_eq!(build(&dir), "v2");
        let reasons: Vec<_> = h.entries().unwrap().into_iter().map(|e| e.reason).collect();
        assert_eq!(reasons, ["first", "second"]);

        let (undone, xml) = h.undo().unwrap();
        assert_eq!(undone.reason, "second");
        assert_eq!(xml, b"v1");
        assert_eq!(build(&dir), "v1");
        h.undo().unwrap();
        assert_eq!(build(&dir), "v0");
        assert!(matches!(h.undo(), Err(HistoryError::Empty)));
        assert!(dir
            .path()
            .join("build-history")
            .read_dir()
            .unwrap()
            .all(|f| { f.unwrap().file_name() == "history.json" }));
    }

    #[test]
    fn a_new_import_ends_the_old_history() {
        let (dir, h) = setup();
        h.record(b"v0", b"v1", "first", json!([]), json!({}))
            .unwrap();
        std::fs::write(dir.path().join("build.xml"), "imported").unwrap();
        assert!(h.entries().unwrap().is_empty());
        assert!(matches!(h.undo(), Err(HistoryError::Changed)));
        assert_eq!(build(&dir), "imported");

        h.record(b"imported", b"i1", "after import", json!([]), json!({}))
            .unwrap();
        let entries = h.entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].reason, "after import");
        assert!(!dir.path().join("build-history/0.xml").exists());
    }

    #[test]
    fn keeps_the_last_entries_only() {
        let (_dir, h) = setup();
        for i in 0..LIMIT + 3 {
            let (a, b) = (format!("v{i}"), format!("v{}", i + 1));
            h.record(
                a.as_bytes(),
                b.as_bytes(),
                &i.to_string(),
                json!([]),
                json!({}),
            )
            .unwrap();
        }
        let entries = h.entries().unwrap();
        assert_eq!(entries.len(), LIMIT);
        assert_eq!(entries[0].reason, "3");
        assert!(!h.snapshot(2).exists());
        assert!(h.snapshot(3).exists());
    }

    #[test]
    fn history_sits_next_to_the_build() {
        let h = BuildHistory::new("/x/guide.xml");
        assert_eq!(h.dir, Path::new("/x/guide-history"));
    }
}
