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
//!   left out and logged. It is uncached still, so a list showing it
//!   after the renderer's retry window hands it in again.
//! - The worker's walk goes through [`check_shared`], so a lookup of
//!   the row the warm is walking joins it, and the warm joins a
//!   lookup's walk of the row it comes to.
//! - The worker starts when the queue gains an item and nobody is
//!   working it, and stops when it is empty; it keeps no state alive
//!   once the queue is drained.

use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::watch;

use super::availability::{cache_key, AvailabilityArgs};
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
        worker::spawn(self.clone(), Arc::clone(state));
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

#[cfg(test)]
impl WarmQueue {
    /// Items waiting their turn.
    pub(crate) fn queued(&self) -> usize {
        self.queue().waiting.len()
    }
}

#[path = "availability_warm_worker.rs"]
mod worker;
