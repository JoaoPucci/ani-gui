//! The warm queue's one worker.

use std::sync::Arc;

use super::{row_of, WarmQueue};
use crate::app::AppState;
use crate::commands::availability::{
    has_usable_row, schedule_seed_ids, warm_backoff, AvailabilityArgs,
};
use crate::commands::availability_flight::check_shared;

/// Start the queue's worker, counted as background AniList traffic.
pub(super) fn spawn(queue: WarmQueue, state: Arc<AppState>) {
    let worker = Worker {
        queue,
        walking: None,
        stopped: false,
    };
    tokio::spawn(crate::meta::anilist_gate::in_background(worker.run(state)));
}

/// The queue's one worker.
struct Worker {
    queue: WarmQueue,
    /// The row being walked, so a worker that unwinds lets go of it.
    walking: Option<String>,
    stopped: bool,
}

impl Worker {
    async fn run(mut self, state: Arc<AppState>) {
        while let Some((seed, args)) = self.queue.next() {
            self.walking = row_of(&args);
            // One AniList request per id space for everything handed in
            // since the last seed, so each probe's per-show seed below
            // is a cache hit (see `schedule_seed_ids`).
            let seed_ids = schedule_seed_ids(&state, &seed);
            crate::commands::airing::seed_airing_rows_batch(&state, &seed_ids, None).await;
            warm_one(&state, args).await;
            if let Some(row) = self.walking.take() {
                self.queue.walked(&row);
            }
        }
        self.stopped = true;
    }
}

impl Drop for Worker {
    /// A worker unwinding from a panic hands the queue back, so the
    /// next push starts another rather than finding it worked forever.
    fn drop(&mut self) {
        if self.stopped {
            return;
        }
        if let Some(row) = self.walking.take() {
            self.queue.walked(&row);
        }
        let mut queue = self.queue.queue();
        queue.working = false;
        self.queue.busy.send_replace(false);
    }
}

/// Probe one item, unless its row is already one the readers serve,
/// then pace.
async fn warm_one(state: &Arc<AppState>, mut args: AvailabilityArgs) {
    let mode = if args.mode == "dub" { "dub" } else { "sub" };
    let Some(id) = args.kitsu_id.clone().filter(|id| !id.is_empty()) else {
        return;
    };
    // Skip entries whose fresh row the readers would serve — the same
    // rule the probe and the batch read apply, so a negative row
    // nobody stands behind is a miss here as it is there.
    if has_usable_row(state, &id, mode) {
        return;
    }
    // Warm probes are background by definition — enforce it
    // server-side regardless of what the caller sent, so the scraper
    // gate paces them and skips them while its breaker is open.
    args.background = true;
    let outcome = check_shared(state, args).await;
    let backoff = warm_backoff(&outcome);
    if let Err(crate::error::AniError::RateLimited {
        retry_after_secs, ..
    }) = &outcome
    {
        tracing::info!(
            retry_after_secs = ?retry_after_secs,
            wait = ?backoff,
            "availability warm: provider rate limited; backing off",
        );
    }
    tokio::time::sleep(backoff).await;
}
