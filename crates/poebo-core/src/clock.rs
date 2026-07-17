//! A pluggable clock so replay pacing is testable.
//!
//! Production uses [`TokioSleeper`] (real time); tests inject [`InstantSleeper`] so replay
//! integration tests run in milliseconds and deterministically.

use std::time::Duration;

use async_trait::async_trait;

/// Something that can asynchronously wait for a duration.
#[async_trait]
pub trait Sleeper: Send + Sync + 'static {
    async fn sleep(&self, dur: Duration);
}

/// Real wall-clock sleeping via Tokio.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioSleeper;

#[async_trait]
impl Sleeper for TokioSleeper {
    async fn sleep(&self, dur: Duration) {
        if !dur.is_zero() {
            tokio::time::sleep(dur).await;
        }
    }
}

/// Test sleeper that returns immediately, so replay pacing collapses to nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct InstantSleeper;

#[async_trait]
impl Sleeper for InstantSleeper {
    async fn sleep(&self, _dur: Duration) {}
}
