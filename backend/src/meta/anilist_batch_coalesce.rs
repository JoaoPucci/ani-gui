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
//! A background lookup the gate would hold back for a turn anyway — a
//! quarter second after the request before it, say — gains nothing by
//! waiting for it in the gate alone. One that finds no lane open then
//! opens one and waits there, and a batch at the end of a window waits
//! there too, until the gate would let a request go: the lookups that
//! arrive meanwhile join it, and it leaves when it would have left
//! alone. The lane holds a place in the gate's queue while it waits
//! ([`crate::meta::anilist_gate::Place`]), so no retry takes the turn
//! ahead of it, and hands that place to the request it sends. A turn
//! past the lookup's wait is not waited for: the gate turns it away at
//! once, as before. The gate's own wait in its log line no longer
//! counts what a lookup waited in its lane.
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
use crate::meta::anilist_gate::{current_class, handing, in_class, place, turn_in, Class, Place};

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
    /// How many lookups wait in `lane`'s next batch now, whatever gate
    /// class they count as. For a test to know every lookup it started
    /// has joined the lane before it lets the lane's window pass.
    #[cfg(test)]
    pub(crate) fn gathered(&self, lane: &str) -> usize {
        let prefix = format!("{lane}\u{0}");
        self.open()
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(_, pending)| pending.len())
            .sum()
    }

    /// How many lanes are open under `lane`, one per gate class.
    #[cfg(test)]
    pub(crate) fn opened(&self, lane: &str) -> usize {
        let prefix = format!("{lane}\u{0}");
        self.open()
            .keys()
            .filter(|key| key.starts_with(&prefix))
            .count()
    }

    fn open(&self) -> std::sync::MutexGuard<'_, Open<T>> {
        self.open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

tokio::task_local! {
    /// Set inside [`gathered`].
    static GATHER: bool;
}

/// Run `f` with every lookup it starts waiting for its lane's window,
/// the one that opens a lane included, so lookups started together go
/// as one batch. For a warm sent before the cards ask: a card's own
/// lookup, outside this, still opens its lane and goes at once, and one
/// that arrives while a gathered lane is open rides its batch, up to the
/// window later than it would have gone alone.
pub(crate) async fn gathered<F: std::future::Future>(f: F) -> F::Output {
    GATHER.scope(true, f).await
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
    // arriving while a lane is open wait for its next batch — and,
    // inside [`gathered`], the one that opens it too.
    let gather = GATHER.try_with(|g| *g).unwrap_or(false);
    let url = base_override
        .unwrap_or(crate::meta::anilist::ANILIST_API)
        .to_owned();
    // Held back for a turn anyway, a lookup waits for it in its lane,
    // where the next ones join it, and leaves when it would have alone.
    let behind = waits_for_turn(class, turn_in(&url, class));
    let (opened, first) = {
        let mut lanes = K::lanes().open();
        match lanes.get_mut(&lane) {
            Some(pending) => {
                pending.push((want, reply));
                (false, None)
            }
            None if gather || behind => {
                lanes.insert(lane.clone(), vec![(want, reply)]);
                (true, None)
            }
            None => {
                lanes.insert(lane.clone(), Vec::new());
                (true, Some(reply))
            }
        }
    };
    if opened {
        // Made before the driver is spawned, so a runtime that ends before
        // the driver first runs still closes the lane.
        let closing = Closing::<K>(Some(lane.clone()), PhantomData);
        if let Some(reply) = first {
            let base = base_override.map(str::to_owned);
            tokio::spawn(in_class(
                class,
                deliver::<K>(client.clone(), base, vec![want], vec![reply]),
            ));
        }
        let driver = Driver::<K> {
            lane,
            client: client.clone(),
            base: base_override.map(str::to_owned),
            url,
            class,
            // Gathered, the lookups started together wait one window;
            // behind the gate's turn, the opener waits only for that.
            first_wait: if behind && !gather {
                Duration::ZERO
            } else {
                WINDOW
            },
            kind: PhantomData,
        };
        tokio::spawn(in_class(class, driver.run(closing)));
    }
    answer.await.unwrap_or(Err(AniError::Network))
}

/// Whether a lookup of `class` whose turn comes in `turn` waits for it in
/// its lane: a background lookup the gate would hold back, inside its
/// wait.
pub(crate) fn waits_for_turn(class: Class, turn: Option<Duration>) -> bool {
    class == Class::Background && turn.is_some_and(|wait| !wait.is_zero())
}

/// What a lane waiting for the gate's turn does next, `left` of its
/// lookups' wait: `Some(nap)` to look again after `nap`, `None` to hand
/// its batch to the gate now. A stub: it waits out every turn inside
/// the class's wait, whatever is left of the lookups'.
pub(crate) fn next_look(class: Class, turn: Option<Duration>, _left: Duration) -> Option<Duration> {
    let turn = turn.filter(|_| waits_for_turn(class, turn))?;
    Some(turn.min(CLOSED_POLL))
}

/// The task gathering one lane's batches.
struct Driver<K> {
    lane: String,
    client: reqwest::Client,
    base: Option<String>,
    /// The endpoint, for its gate.
    url: String,
    class: Class,
    /// How long the first batch gathers before it waits for its turn.
    first_wait: Duration,
    kind: PhantomData<K>,
}

impl<K: Kind> Driver<K> {
    /// Each window, hand what the lane gathered to a task of its own and
    /// go on gathering: a lookup arriving while a batch is out goes in
    /// the next window's batch, not after the first one answers.
    async fn run(self, mut guard: Closing<K>) {
        let mut wait = self.first_wait;
        loop {
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
            wait = WINDOW;
            let Some(place) = self.until_turn().await else {
                guard.0 = None;
                return;
            };
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
                handing(
                    place,
                    tokio::time::Instant::now(),
                    deliver::<K>(client, base, wants, replies),
                ),
            ));
        }
    }

    /// Wait, while what the lane gathered would be held back for a turn
    /// inside its wait, for that turn, holding a place in the gate's
    /// queue; callers that stop waiting meanwhile are let go. The place,
    /// if one was taken; `None`, and the lane closed, once nobody waits.
    async fn until_turn(&self) -> Option<Option<Place>> {
        let mut held = None;
        loop {
            if !self.still_wanted() {
                return None;
            }
            let turn = turn_in(&self.url, self.class);
            if !waits_for_turn(self.class, turn) {
                return Some(held);
            }
            held.get_or_insert_with(|| place(&self.url));
            let wait = turn.unwrap_or_default();
            tokio::time::sleep(wait.min(CLOSED_POLL)).await;
        }
    }

    /// Drop the gathered lookups whose callers stopped waiting; whether
    /// any is left. A lane left with none is closed.
    fn still_wanted(&self) -> bool {
        let mut lanes = K::lanes().open();
        let Some(pending) = lanes.get_mut(&self.lane) else {
            return false;
        };
        pending.retain(|(_, reply)| !reply.is_closed());
        if pending.is_empty() {
            lanes.remove(&self.lane);
            return false;
        }
        true
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

#[cfg(test)]
#[path = "anilist_batch_coalesce_prop_test.rs"]
mod prop_tests;
