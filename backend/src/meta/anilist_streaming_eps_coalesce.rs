//! Streaming-listing lookups that miss the cache together share
//! AniList's requests.
//!
//! Home's Continue Watching strip asks for each card's episodes as the
//! card's show resolves, one route call per card, and each call fills
//! its stills from AniList on a cache miss. Each used to send its own
//! `Media` query, so a cold strip of ten shows spent ten requests of
//! AniList's allowance in one burst, and the gate held most of them
//! back. Here a lookup joins a lane instead: the first opens it and,
//! after [`WINDOW`], the lane's lookups go out as one batch
//! ([`super::streaming_listings_batch`]); lookups arriving while a batch
//! is out wait for the next. A lane holding one lookup sends the single
//! query it always sent.
//!
//! Lanes are kept apart by the caller's key (its app state and AniList
//! endpoint) and by the gate class the lookups count as, so a retry of a
//! held-back fill is never sent as a background request, or the other
//! way round. The batch runs in a task of its own, so a caller that
//! stops waiting — the episode route gives the fill a budget — does not
//! take the others' answers with it; a caller gone before its batch went
//! out is left out of it. What each caller does with its answer —
//! caching it, or queueing a retry — stays in its own task.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;

use super::{streaming_listing_for_ids, streaming_listings_batch, StreamingListing, Want};
use crate::error::{AniError, Result};
use crate::meta::anilist_gate::{current_class, in_class};

/// How long a lane waits for more lookups before its batch goes out:
/// a fraction of the gate's least gap between two requests, and of the
/// episode route's budget for the fill.
pub(crate) const WINDOW: Duration = Duration::from_millis(100);

type Reply = oneshot::Sender<Result<StreamingListing>>;

/// Each open lane's lookups not yet sent, by lane.
type Lanes = HashMap<String, Vec<(Want, Reply)>>;

/// The lanes with a batch being gathered or sent, by key; a lane is
/// here exactly while its driver runs.
static LANES: LazyLock<Mutex<Lanes>> = LazyLock::new(Default::default);

fn lanes() -> std::sync::MutexGuard<'static, Lanes> {
    LANES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// `want`'s streaming listing, asked with the other lookups of `lane`
/// that miss at the same time.
///
/// # Errors
/// As [`super::streaming_listings_batch`] for the show;
/// [`AniError::Network`] when the batch was lost before it answered.
pub(crate) async fn streaming_listing_coalesced(
    client: &reqwest::Client,
    lane: &str,
    want: Want,
    base_override: Option<&str>,
) -> Result<StreamingListing> {
    let class = current_class();
    let lane = format!("{lane}\u{0}{class:?}");
    let (reply, answer) = oneshot::channel();
    let opened = {
        let mut lanes = lanes();
        match lanes.get_mut(&lane) {
            Some(pending) => {
                pending.push((want, reply));
                false
            }
            None => {
                lanes.insert(lane.clone(), vec![(want, reply)]);
                true
            }
        }
    };
    if opened {
        let driver = Driver {
            lane,
            client: client.clone(),
            base: base_override.map(str::to_owned),
        };
        tokio::spawn(in_class(class, driver.run()));
    }
    answer.await.unwrap_or(Err(AniError::Network))
}

/// The task sending one lane's batches.
struct Driver {
    lane: String,
    client: reqwest::Client,
    base: Option<String>,
}

impl Driver {
    async fn run(self) {
        let mut guard = Closing(Some(self.lane.clone()));
        loop {
            tokio::time::sleep(WINDOW).await;
            let Some(batch) = self.take() else {
                guard.0 = None;
                return;
            };
            let (wants, replies): (Vec<Want>, Vec<Reply>) = batch
                .into_iter()
                .filter(|(_, reply)| !reply.is_closed())
                .unzip();
            let answers = self.send(&wants).await;
            for (reply, answer) in replies.into_iter().zip(answers) {
                let _ = reply.send(answer);
            }
        }
    }

    /// The lookups gathered since the last batch; `None`, and the lane
    /// closed, when there are none.
    fn take(&self) -> Option<Vec<(Want, Reply)>> {
        let mut lanes = lanes();
        let pending = lanes.get_mut(&self.lane)?;
        if pending.is_empty() {
            lanes.remove(&self.lane);
            return None;
        }
        Some(std::mem::take(pending))
    }

    async fn send(&self, wants: &[Want]) -> Vec<Result<StreamingListing>> {
        let base = self.base.as_deref();
        match wants {
            [] => Vec::new(),
            [one] => {
                vec![streaming_listing_for_ids(&self.client, one.mal, one.anilist, base).await]
            }
            _ => streaming_listings_batch(&self.client, wants, base).await,
        }
    }
}

/// Closes the lane when its driver ends without closing it — dropped
/// with its runtime, or unwinding — so the lookups still in it fail
/// rather than wait on a driver that is gone, and the next lookup opens
/// the lane again.
struct Closing(Option<String>);

impl Drop for Closing {
    fn drop(&mut self) {
        if let Some(lane) = self.0.take() {
            lanes().remove(&lane);
        }
    }
}
