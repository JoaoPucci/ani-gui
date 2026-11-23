//! Lookups of one [`Kind`] that miss the cache together share AniList's
//! requests.
//!
//! Home's Continue Watching strip reads each card's episodes and detail
//! as the card's show resolves, one route call per card, and each call
//! fills the card's stills, or a coverless show's banner, from AniList
//! on a cache miss. Each used to send its own `Media` query, so a strip
//! of ten shows spent ten requests of AniList's allowance in one burst,
//! and the gate held about half of them back.
//!
//! Here a lookup joins a lane instead. One that finds no lane open opens
//! one and goes out at once, alone, with the single query it always
//! sent: what a card's look waits on is never held for a window. Lookups
//! arriving while the lane is open gather, and every [`WINDOW`] what the
//! lane gathered goes out as one batch ([`super::batch_each`]) in a task
//! of its own, without waiting for the batch before it; a window that
//! gathers nothing closes the lane. Each caller is answered as soon as
//! its own answer is in.
//!
//! Lanes are kept apart by kind, by the caller's key (its app state and
//! AniList endpoint) and by the gate class the lookups count as, so a
//! retry of a held-back fill is never sent as a background request, or
//! the other way round. A caller that stops waiting — the episode route
//! gives its fill a budget — does not take the others' answers with it;
//! a caller gone before its batch was gathered is left out of it, and a
//! batch no caller waits on any more is dropped, out of the gate's queue
//! if it is still waiting for its turn. What each caller does with its
//! answer — caching it, or queueing a retry — stays in its own task.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::oneshot;

use super::{batch_each, Kind, Want};
use crate::error::{AniError, Result};
use crate::meta::anilist_gate::{current_class, in_class, Class};

/// How long a lane gathers lookups before its batch goes out: under the
/// gate's least gap between two requests (250 ms), so the MAL-id page a
/// gathered lookup rides in leaves no later than its own request would
/// have, the lane's first having just been sent. A show asked on the
/// AniList-id page waits for the MAL page's answer first.
pub(crate) const WINDOW: Duration = Duration::from_millis(100);

type Reply<T> = oneshot::Sender<Result<T>>;

/// Each open lane's lookups not yet sent, by lane.
type Open<T> = HashMap<String, Vec<(Want, Reply<T>)>>;

/// A kind's open lanes: each one's lookups not yet sent, by lane. A lane
/// is here exactly while its driver runs.
pub(crate) struct Lanes<T> {
    open: Mutex<Open<T>>,
}

impl<T> Default for Lanes<T> {
    fn default() -> Self {
        Self {
            open: Mutex::new(HashMap::new()),
        }
    }
}

impl<T> Lanes<T> {
    fn open(&self) -> std::sync::MutexGuard<'_, Open<T>> {
        self.open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// `want`'s answer of kind `K`, asked with the other lookups of `lane`
/// that miss at the same time.
///
/// # Errors
/// As [`batch`] for the show; [`AniError::Network`] when the batch was
/// lost before it answered.
pub(crate) async fn coalesced<K: Kind>(
    client: &reqwest::Client,
    lane: &str,
    want: Want,
    base_override: Option<&str>,
) -> Result<K::Out> {
    let class = current_class();
    let lane = format!("{lane}\u{0}{class:?}");
    let (reply, answer) = oneshot::channel();
    // The lookup that opens the lane goes out at once, alone: what a
    // card's look waits on is never held for a window. Only lookups
    // arriving while a lane is open wait for its next batch.
    let first = {
        let mut lanes = K::lanes().open();
        match lanes.get_mut(&lane) {
            Some(pending) => {
                pending.push((want, reply));
                None
            }
            None => {
                lanes.insert(lane.clone(), Vec::new());
                Some(reply)
            }
        }
    };
    if let Some(reply) = first {
        // Made before the driver is spawned, so a runtime that ends before
        // the driver first runs still closes the lane.
        let closing = Closing::<K>(Some(lane.clone()), PhantomData);
        let base = base_override.map(str::to_owned);
        tokio::spawn(in_class(
            class,
            deliver::<K>(client.clone(), base, vec![want], vec![reply]),
        ));
        let driver = Driver::<K> {
            lane,
            client: client.clone(),
            base: base_override.map(str::to_owned),
            class,
            kind: PhantomData,
        };
        tokio::spawn(in_class(class, driver.run(closing)));
    }
    answer.await.unwrap_or(Err(AniError::Network))
}

/// The task gathering one lane's batches.
struct Driver<K> {
    lane: String,
    client: reqwest::Client,
    base: Option<String>,
    class: Class,
    kind: PhantomData<K>,
}

impl<K: Kind> Driver<K> {
    /// Each window, hand what the lane gathered to a task of its own and
    /// go on gathering: a lookup arriving while a batch is out goes in
    /// the next window's batch, not after the first one answers.
    async fn run(self, mut guard: Closing<K>) {
        loop {
            tokio::time::sleep(WINDOW).await;
            let Some(batch) = self.take() else {
                guard.0 = None;
                return;
            };
            let (wants, replies): (Vec<Want>, Vec<Reply<K::Out>>) = batch
                .into_iter()
                .filter(|(_, reply)| !reply.is_closed())
                .unzip();
            let client = self.client.clone();
            let base = self.base.clone();
            tokio::spawn(in_class(
                self.class,
                deliver::<K>(client, base, wants, replies),
            ));
        }
    }

    /// The lookups gathered since the last batch; `None`, and the lane
    /// closed, when there are none.
    fn take(&self) -> Option<Vec<(Want, Reply<K::Out>)>> {
        let mut lanes = K::lanes().open();
        let pending = lanes.get_mut(&self.lane)?;
        if pending.is_empty() {
            lanes.remove(&self.lane);
            return None;
        }
        Some(std::mem::take(pending))
    }
}

/// Send one batch, answering each caller as soon as its own answer is in
/// — a caller that stops waiting keeps what was answered before it left.
/// Once no caller waits any more the batch is dropped, out of the gate's
/// queue if it is still waiting for its turn, rather than sent for
/// nobody.
async fn deliver<K: Kind>(
    client: reqwest::Client,
    base: Option<String>,
    wants: Vec<Want>,
    replies: Vec<Reply<K::Out>>,
) {
    let replies = Mutex::new(replies.into_iter().map(Some).collect::<Vec<_>>());
    let answer = |index: usize, answer: Result<K::Out>| {
        let reply = replies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_mut(index)
            .and_then(Option::take);
        if let Some(reply) = reply {
            let _ = reply.send(answer);
        }
    };
    tokio::select! {
        () = send::<K>(&client, base.as_deref(), &wants, answer) => {}
        () = all_closed(&replies) => {}
    }
}

/// How often a batch checks whether any caller still waits on it.
const CLOSED_POLL: Duration = Duration::from_millis(20);

/// Resolves once no caller still waits on an answer from the batch.
async fn all_closed<T>(replies: &Mutex<Vec<Option<Reply<T>>>>) {
    loop {
        let waiting = replies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .flatten()
            .any(|reply| !reply.is_closed());
        if !waiting {
            return;
        }
        tokio::time::sleep(CLOSED_POLL).await;
    }
}

async fn send<K: Kind>(
    client: &reqwest::Client,
    base: Option<&str>,
    wants: &[Want],
    mut answer: impl FnMut(usize, Result<K::Out>),
) {
    match wants {
        [] => {}
        [one] => answer(0, super::single::<K>(client, *one, base).await),
        _ => batch_each::<K>(client, wants, base, answer).await,
    }
}

/// Closes the lane when its driver ends without closing it — dropped
/// with its runtime, or unwinding — so the lookups still in it fail
/// rather than wait on a driver that is gone, and the next lookup opens
/// the lane again.
struct Closing<K: Kind>(Option<String>, PhantomData<K>);

impl<K: Kind> Drop for Closing<K> {
    fn drop(&mut self) {
        if let Some(lane) = self.0.take() {
            K::lanes().open().remove(&lane);
        }
    }
}
