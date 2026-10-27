//! The availability warm: what a list view hands the backend to probe
//! in the background, and the one worker that probes it.
//!
//! Every Home mount warms its rails, and each warm used to spawn a
//! task of its own that nothing deduped or cancelled — so going Home,
//! into a show and back again stacked one walk per mount for every row
//! still uncached, the same row warmed several times over at once.
//! Now a warm item joins a queue held per state, and one worker walks
//! it a row at a time:
//!
//! - A row already queued, or being walked, is not queued again.
//! - The queue holds at most [`WARM_QUEUE_MAX`] rows. One past that is
//!   left out and logged; it is uncached still, so the next list that
//!   shows it hands it in again.
//! - The worker's walk goes through [`check_shared`], so a lookup of
//!   the row the warm is walking joins it, and the warm joins a
//!   lookup's walk of the row it comes to.
//! - The worker starts when the queue gains an item and nobody is
//!   working it, and stops when it is empty; it keeps no state alive
//!   once the queue is drained.

use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::watch;

use super::availability::{
    cache_key, has_usable_row, schedule_seed_ids, warm_backoff, AvailabilityArgs,
};
use super::availability_flight::check_shared;
use crate::app::AppState;

/// How many warm items may wait their turn: a Home's rails several
/// times over, so a full queue is a sign of nothing walking rather
/// than of a page with many cards.
pub(crate) const WARM_QUEUE_MAX: usize = 256;

#[derive(Default)]
struct Queue {
    waiting: VecDeque<AvailabilityArgs>,
    /// The rows waiting, and the one being walked.
    rows: HashSet<String>,
    /// Items handed in whose airing rows are not seeded yet.
    unseeded: Vec<AvailabilityArgs>,
    working: bool,
}

/// The warm items handed in and not yet walked. One per app state; a
/// clone shares the queue.
#[derive(Clone)]
pub struct WarmQueue {
    queue: Arc<Mutex<Queue>>,
    /// Whether a worker is out, for whoever waits for the queue to
    /// drain.
    busy: Arc<watch::Sender<bool>>,
}

impl Default for WarmQueue {
    fn default() -> Self {
        Self {
            queue: Arc::default(),
            busy: Arc::new(watch::channel(false).0),
        }
    }
}

/// The row an item warms, when it names one.
fn row_of(args: &AvailabilityArgs) -> Option<String> {
    let id = args.kitsu_id.as_deref().filter(|id| !id.is_empty())?;
    Some(cache_key(id, &args.mode))
}

impl WarmQueue {
    fn queue(&self) -> std::sync::MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Hand `items` to the warm. Returns at once.
    pub fn push(&self, state: &Arc<AppState>, items: Vec<AvailabilityArgs>) {
        let mut queue = self.queue();
        let mut left_out = 0usize;
        for args in items {
            let Some(row) = row_of(&args) else {
                continue;
            };
            if queue.rows.contains(&row) {
                continue;
            }
            if queue.waiting.len() >= WARM_QUEUE_MAX {
                left_out += 1;
                continue;
            }
            queue.rows.insert(row);
            queue.unseeded.push(args.clone());
            queue.waiting.push_back(args);
        }
        if left_out > 0 {
            tracing::info!(
                left_out,
                "availability warm: queue full; the rows left out are warmed when a list hands them in again",
            );
        }
        if queue.working || queue.waiting.is_empty() {
            return;
        }
        queue.working = true;
        self.busy.send_replace(true);
        drop(queue);
        let worker = Worker {
            queue: self.clone(),
            walking: None,
            stopped: false,
        };
        tokio::spawn(crate::meta::anilist_gate::in_background(
            worker.run(Arc::clone(state)),
        ));
    }

    /// Returns once no worker is out.
    pub async fn drained(&self) {
        let mut busy = self.busy.subscribe();
        // The sender lives in `self`, so the wait cannot be closed.
        let _ = busy.wait_for(|busy| !*busy).await;
    }

    /// The next item, with the items to seed before it; `None`, and
    /// the worker stopped, when the queue is empty.
    fn next(&self) -> Option<(Vec<AvailabilityArgs>, AvailabilityArgs)> {
        let mut queue = self.queue();
        let seed = std::mem::take(&mut queue.unseeded);
        if let Some(next) = queue.waiting.pop_front() {
            return Some((seed, next));
        }
        queue.working = false;
        self.busy.send_replace(false);
        None
    }

    fn walked(&self, row: &str) {
        self.queue().rows.remove(row);
    }
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

#[cfg(test)]
impl WarmQueue {
    /// Items waiting their turn.
    pub(crate) fn queued(&self) -> usize {
        self.queue().waiting.len()
    }
}
