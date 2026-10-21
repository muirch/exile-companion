//! Replay pacing goes through [`Sleeper`] so tests can swap in [`InstantSleeper`].

use std::future::Future;
use std::time::Duration;

pub trait Sleeper: Send + Sync + 'static {
    fn sleep(&self, dur: Duration) -> impl Future<Output = ()> + Send;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TokioSleeper;

impl Sleeper for TokioSleeper {
    async fn sleep(&self, dur: Duration) {
        if !dur.is_zero() {
            tokio::time::sleep(dur).await;
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct InstantSleeper;

impl Sleeper for InstantSleeper {
    async fn sleep(&self, _dur: Duration) {}
}
