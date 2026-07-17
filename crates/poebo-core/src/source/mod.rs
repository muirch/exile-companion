//! The source abstraction that unifies live tailing and replay.
//!
//! Both [`live::LiveTail`] and [`replay::ReplaySource`] implement [`EventSource`], producing a
//! [`LineStream`] of decoded [`RawLine`]s. The pipeline consumes either without knowing which - this
//! is the seam that makes replay behaviorally identical to live and lets Phase 2 inputs drop in as
//! new producers.

use std::pin::Pin;

use futures::Stream;

use crate::events::RawLine;

pub mod live;
pub mod replay;

pub use crate::error::SourceError;
pub use live::LiveTail;
pub use replay::{ReplayControl, ReplaySource, SpeedMultiplier};

/// A stream of decoded log lines. Parsing and state folding happen downstream, identically for
/// every source.
pub type LineStream = Pin<Box<dyn Stream<Item = Result<RawLine, SourceError>> + Send>>;

/// Anything that can produce a stream of decoded log lines.
pub trait EventSource {
    fn into_stream(self: Box<Self>) -> LineStream;
}
