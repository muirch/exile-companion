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

/// Where the player is by the end of the log: character, level, act and area.
pub fn state_from_log(
    path: &std::path::Path,
    zones: std::sync::Arc<crate::data::ZoneTable>,
    locale: &LocalePack,
) -> std::io::Result<CampaignState> {
    let text = crate::logfile::read_tail(path, crate::logfile::HISTORY_BYTES)?;
    let mut machine = CampaignMachine::new(zones);
    prime_from_history(&mut machine, &text, locale);
    Ok(machine.into_state())
}

/// Folds already-written log text into `machine`, so the overlay starts from where the player is
/// instead of waiting for the next zone change. The first line may be cut, which is fine:
/// `split_line` rejects it.
pub fn prime_from_history(machine: &mut CampaignMachine, text: &str, locale: &LocalePack) {
    for line in text.lines() {
        let Some(raw) = crate::parser::split_line(line) else {
            continue;
        };
        if let Some(event) = classify(&raw.body, locale) {
            machine.apply(event, raw.ts);
        }
    }
}
