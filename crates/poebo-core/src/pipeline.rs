//! The single, source-agnostic consumer: parse -> fold into campaign state -> notify.
//!
//! `run_pipeline` drives any [`LineStream`] (live or replay) through the exact same steps, so the
//! two are behaviorally identical. It optionally tees each raw line to a [`Recorder`], and invokes
//! `on_update` only when a meaningful [`StateDelta`] occurs (so the UI isn't spammed on re-entry of
//! the same zone).

use futures::StreamExt;

use crate::error::PipelineError;
use crate::locale::LocalePack;
use crate::parser::classify;
use crate::recorder::Recorder;
use crate::source::LineStream;
use crate::state::{CampaignMachine, CampaignState, StateDelta};

/// Run the parse -> state pipeline to completion (or until the source ends / errors).
///
/// `on_update(state, delta)` fires on every meaningful change. Recording (if `tee` is set) writes
/// every raw line, including ones that classify as noise, so recordings are faithful.
pub async fn run_pipeline<F>(
    mut stream: LineStream,
    mut machine: CampaignMachine,
    mut tee: Option<Recorder>,
    locale: LocalePack,
    mut on_update: F,
) -> Result<(), PipelineError>
where
    F: FnMut(&CampaignState, &StateDelta) + Send,
{
    while let Some(item) = stream.next().await {
        let raw = item?;

        if let Some(recorder) = tee.as_mut() {
            recorder.write_line(&raw.raw).await?;
        }

        if let Some(event) = classify(&raw.body, &locale) {
            let delta = machine.apply(event, raw.ts);
            if delta.is_meaningful() {
                on_update(machine.snapshot(), &delta);
            }
        }
    }

    if let Some(mut recorder) = tee {
        recorder.flush().await?;
    }
    Ok(())
}
