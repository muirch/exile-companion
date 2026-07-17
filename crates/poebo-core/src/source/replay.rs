//! Replay a recorded `Client.txt` through the same pipeline as live tailing.
//!
//! [`ReplaySource`] reads a file, then releases its lines paced by the timestamp deltas between
//! them (divided by the current speed, clamped). [`ReplayControl`] — a shared, thread-safe handle —
//! drives pause/resume/step/speed/stop while the stream runs. Pacing goes through a [`Sleeper`] so
//! tests collapse it to zero.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering::SeqCst};
use std::sync::Arc;
use std::time::Duration;

use time::PrimitiveDateTime;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use super::{EventSource, LineStream};
use crate::clock::Sleeper;
use crate::events::RawLine;

/// Replay speed multipliers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SpeedMultiplier {
    #[default]
    X1,
    X2,
    X5,
    X20,
    X100,
}

impl SpeedMultiplier {
    /// The divisor applied to inter-line gaps.
    pub fn factor(self) -> u32 {
        match self {
            SpeedMultiplier::X1 => 1,
            SpeedMultiplier::X2 => 2,
            SpeedMultiplier::X5 => 5,
            SpeedMultiplier::X20 => 20,
            SpeedMultiplier::X100 => 100,
        }
    }

    /// Nearest multiplier for a raw factor (defaults to `X1`).
    pub fn from_factor(factor: u32) -> Self {
        match factor {
            2 => SpeedMultiplier::X2,
            5 => SpeedMultiplier::X5,
            20 => SpeedMultiplier::X20,
            100 => SpeedMultiplier::X100,
            _ => SpeedMultiplier::X1,
        }
    }
}

/// Largest real-time gap we ever wait between two lines, regardless of the log delta.
const MAX_GAP: Duration = Duration::from_secs(5);

/// A shared, thread-safe handle controlling a running replay.
#[derive(Debug)]
pub struct ReplayControl {
    speed: AtomicU32,
    paused: AtomicBool,
    step_budget: AtomicUsize,
    position: AtomicUsize,
    total: AtomicUsize,
    notify: Notify,
    stop: CancellationToken,
}

impl ReplayControl {
    /// Create a control handle at the given initial speed.
    pub fn new(initial_speed: SpeedMultiplier) -> Arc<Self> {
        Arc::new(Self {
            speed: AtomicU32::new(initial_speed.factor()),
            paused: AtomicBool::new(false),
            step_budget: AtomicUsize::new(0),
            position: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
            notify: Notify::new(),
            stop: CancellationToken::new(),
        })
    }

    pub fn set_speed(&self, speed: SpeedMultiplier) {
        self.speed.store(speed.factor(), SeqCst);
        self.notify.notify_waiters();
    }

    /// Current speed divisor (>= 1).
    pub fn speed(&self) -> u32 {
        self.speed.load(SeqCst).max(1)
    }

    pub fn pause(&self) {
        self.paused.store(true, SeqCst);
        self.notify.notify_waiters();
    }

    pub fn resume(&self) {
        self.paused.store(false, SeqCst);
        self.notify.notify_waiters();
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(SeqCst)
    }

    /// Release exactly one more line while paused.
    pub fn step(&self) {
        self.step_budget.fetch_add(1, SeqCst);
        self.notify.notify_waiters();
    }

    /// End the replay; the stream terminates promptly.
    pub fn stop(&self) {
        self.stop.cancel();
        self.notify.notify_waiters();
    }

    /// Lines released so far.
    pub fn position(&self) -> usize {
        self.position.load(SeqCst)
    }

    /// Total lines to release.
    pub fn total(&self) -> usize {
        self.total.load(SeqCst)
    }
}

enum Gate {
    Go,
    Stop,
}

/// Block while paused (releasing one line per `step`); return `Stop` if cancelled.
async fn gate(ctl: &ReplayControl) -> Gate {
    loop {
        if ctl.stop.is_cancelled() {
            return Gate::Stop;
        }
        if !ctl.paused.load(SeqCst) {
            return Gate::Go;
        }
        if ctl.step_budget.load(SeqCst) > 0 {
            ctl.step_budget.fetch_sub(1, SeqCst);
            return Gate::Go;
        }
        tokio::select! {
            _ = ctl.notify.notified() => {}
            _ = ctl.stop.cancelled() => return Gate::Stop,
        }
    }
}

/// Compute the real-time delay before releasing a line, from the log timestamp gap and speed.
fn compute_delay(
    prev: Option<PrimitiveDateTime>,
    cur: Option<PrimitiveDateTime>,
    speed: u32,
) -> Duration {
    match (prev, cur) {
        (Some(p), Some(c)) if c > p => {
            let secs = (c - p).as_seconds_f64() / f64::from(speed.max(1));
            Duration::try_from_secs_f64(secs.max(0.0))
                .unwrap_or(Duration::ZERO)
                .min(MAX_GAP)
        }
        _ => Duration::ZERO,
    }
}

/// Replays a recorded log file with controllable pacing.
pub struct ReplaySource<S: Sleeper> {
    path: PathBuf,
    ctl: Arc<ReplayControl>,
    sleeper: S,
}

impl<S: Sleeper> ReplaySource<S> {
    pub fn new(path: impl Into<PathBuf>, ctl: Arc<ReplayControl>, sleeper: S) -> Self {
        Self {
            path: path.into(),
            ctl,
            sleeper,
        }
    }
}

impl<S: Sleeper> EventSource for ReplaySource<S> {
    fn into_stream(self: Box<Self>) -> LineStream {
        let ReplaySource { path, ctl, sleeper } = *self;

        Box::pin(async_stream::try_stream! {
            let content = tokio::fs::read_to_string(&path).await?;
            let raws: Vec<RawLine> = content.lines().filter_map(crate::parser::split_line).collect();
            ctl.total.store(raws.len(), SeqCst);
            ctl.position.store(0, SeqCst);

            let mut prev_ts: Option<PrimitiveDateTime> = None;
            for (i, raw) in raws.into_iter().enumerate() {
                if let Gate::Stop = gate(&ctl).await {
                    break;
                }
                let delay = compute_delay(prev_ts, raw.ts, ctl.speed());
                tokio::select! {
                    _ = sleeper.sleep(delay) => {}
                    _ = ctl.stop.cancelled() => break,
                }
                prev_ts = raw.ts.or(prev_ts);
                ctl.position.store(i + 1, SeqCst);
                yield raw;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_factor_roundtrip() {
        for s in [
            SpeedMultiplier::X1,
            SpeedMultiplier::X2,
            SpeedMultiplier::X5,
            SpeedMultiplier::X20,
            SpeedMultiplier::X100,
        ] {
            assert_eq!(SpeedMultiplier::from_factor(s.factor()), s);
        }
    }

    #[test]
    fn delay_scales_with_speed_and_clamps() {
        use time::macros::datetime;
        let a = datetime!(2020-01-01 00:00:00);
        let b = datetime!(2020-01-01 00:00:10); // 10s gap
        assert_eq!(compute_delay(Some(a), Some(b), 1), Duration::from_secs(5)); // clamped to MAX_GAP
        assert_eq!(compute_delay(Some(a), Some(b), 10), Duration::from_secs(1));
        assert_eq!(compute_delay(None, Some(b), 1), Duration::ZERO);
        // Out-of-order timestamps never produce a negative delay.
        assert_eq!(compute_delay(Some(b), Some(a), 1), Duration::ZERO);
    }
}
