//! The line a lane logs for each request it sends: one INFO line, once
//! the answer lands, so the app's log shows the batching at work. It
//! names the kind, which request, how many ids it carried, how it ended
//! and how long it took — never a show's id. Split from
//! `anilist_batch.rs` so each file stays inside the CRAP gate's per-file
//! bar.

use std::future::Future;
use std::time::Instant;

use crate::error::Result;

/// Which request a lane sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sent {
    /// One show's single lookup.
    Single,
    /// A `Page` of MAL ids.
    MalPage,
    /// A `Page` of AniList ids.
    AniListIdPage,
}

impl Sent {
    fn name(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::MalPage => "mal_page",
            Self::AniListIdPage => "anilist_id_page",
        }
    }
}

/// Await `request` — the `sent` request of a `kind` lane, carrying `ids`
/// ids — and log its line once it is answered, or turned away unsent by
/// the gate (its status the rate-limit key). `ms` counts from when the
/// lane handed it to the gate, its wait for a turn included. A single
/// lookup that asks again by AniList id is one line.
pub(crate) async fn logged<T, F: Future<Output = Result<T>>>(
    kind: &'static str,
    sent: Sent,
    ids: usize,
    request: F,
) -> Result<T> {
    let began = Instant::now();
    let answer = request.await;
    let status = match &answer {
        Ok(_) => "ok",
        Err(e) => e.key(),
    };
    tracing::info!(
        kind,
        request = sent.name(),
        ids,
        status,
        ms = u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX),
        "anilist lane request answered"
    );
    answer
}

#[cfg(test)]
#[path = "anilist_batch_log_test.rs"]
mod tests;
