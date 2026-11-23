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
//! Here a lookup joins a lane instead. One that finds no lane open, when
//! the gate would let it go now, opens one and goes out at once, alone,
//! with the single query it always sent: what a card's look waits on is
//! never held for a window. Lookups
//! arriving while the lane is open gather, and every [`WINDOW`] what the
//! lane gathered goes out as one batch ([`super::batch_each`]) in a task
//! of its own, without waiting for the batch before it; a window that
//! gathers nothing closes the lane. Each caller is answered as soon as
//! its own answer is in.
//!
//! A background lookup the gate would hold back for a turn anyway — a
//! quarter second after the request before it, say — gains nothing by
//! waiting for it in the gate alone. One with a MAL id that finds no
//! lane open then opens one and waits there, and a batch at the end of
//! a window waits there too, until the gate's turn is a moment
//! ([`CLOSED_POLL`]) away: the MAL lookups that arrive meanwhile join
//! it, and the batch waits out the rest in the gate's own queue, so it
//! leaves at the turn it would have taken alone. The lane holds a place
//! in the gate's queue while it waits
//! ([`crate::meta::anilist_gate::Place`]), so no retry takes the turn
//! ahead of it, and hands that place to the request it sends. Its wait
//! counts against the lookups' own, from when it began or its last
//! lookup joined, whichever is later: past it the batch is handed over
//! with that deadline, and the gate turns it away, as it would have
//! turned that last lookup away waiting there alone. An earlier lookup
//! may so wait past its own wait, never stop short of it.
//!
//! A show Kitsu maps to AniList alone is never held this way, and never
//! joins a held lane: a page of AniList ids goes only once the MAL page
//! beside it has answered, so in a held batch it would leave later than
//! alone. A batch at the end of a window that carries one is handed
//! over at once, as before. One left is a show two AniList media record
//! under one MAL id, which its page hands to a single lookup: in a held
//! batch that lookup goes after the page. The gate's own wait in its
//! log line no longer counts what a lookup waited in its lane.
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
use crate::meta::anilist_gate::{current_class, in_class, turn_in};
use tokio::time::Instant;

/// How long a lane gathers lookups before its batch goes out: under the
/// gate's least gap between two requests (250 ms), so the MAL-id page a
/// gathered lookup rides in leaves no later than its own request would
/// have, the lane's first having just been sent. A show asked on the
/// AniList-id page waits for the MAL page's answer first.
pub(crate) const WINDOW: Duration = Duration::from_millis(100);

type Reply<T> = oneshot::Sender<Result<T>>;

/// Each open lane's lookups not yet sent, by lane.
type Open<T> = HashMap<String, Gathering<T>>;

/// One open lane's lookups not yet sent.
struct Gathering<T> {
    pending: Vec<(Want, Reply<T>)>,
    /// Waiting for the gate's turn rather than for its window. A lookup
    /// Kitsu maps to AniList alone does not join it: its page goes only
    /// once the MAL page has answered, later than it would have alone.
    held: bool,
    /// When the last lookup joined: a held batch waits no longer than
    /// that lookup's own wait would have let it.
    last_join: Instant,
}

impl<T> Gathering<T> {
    fn new(pending: Vec<(Want, Reply<T>)>, held: bool) -> Self {
        Self {
            pending,
            held,
            last_join: Instant::now(),
        }
    }

    fn join(&mut self, want: Want, reply: Reply<T>) {
        self.pending.push((want, reply));
        self.last_join = Instant::now();
    }
}

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
            .map(|(_, lane)| lane.pending.len())
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
/// lookup, outside this, still opens its lane and goes at once when the
/// gate would let it go, and one that arrives while a gathered lane is
/// open rides its batch, up to the window later than it would have gone
/// alone. A gathered batch then waits for the gate's turn as any other
/// does.
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
    // The lookup that opens the lane goes out at once, alone, when the
    // gate would let it go: what a card's look waits on is never held
    // for a window. Only lookups arriving while a lane is open wait for
    // its next batch — and, inside [`gathered`], the one that opens it
    // too.
    let gather = GATHER.try_with(|g| *g).unwrap_or(false);
    let url = base_override
        .unwrap_or(crate::meta::anilist::ANILIST_API)
        .to_owned();
    // Held back for a turn anyway, a lookup with a MAL id waits for it
    // in its lane, where the next ones join it, and leaves when it would
    // have alone. One Kitsu maps to AniList alone goes as before: in a
    // page it would wait for the MAL page's answer.
    let by_mal = want.mal.is_some();
    let behind = by_mal && waits_for_turn(class, turn_in(&url, class));
    let (opened, first) = {
        let mut lanes = K::lanes().open();
        match lanes.get_mut(&lane) {
            Some(open) if by_mal || !open.held => {
                open.join(want, reply);
                (false, None)
            }
            // Beside a lane held for the gate's turn: alone, at once.
            Some(_) => (false, Some(reply)),
            None if gather || behind => {
                lanes.insert(lane.clone(), Gathering::new(vec![(want, reply)], behind));
                (true, None)
            }
            None => {
                lanes.insert(lane.clone(), Gathering::new(Vec::new(), false));
                (true, Some(reply))
            }
        }
    };
    if let Some(reply) = first {
        let base = base_override.map(str::to_owned);
        tokio::spawn(in_class(
            class,
            deliver::<K>(client.clone(), base, vec![want], vec![reply]),
        ));
    }
    if opened {
        // Made before the driver is spawned, so a runtime that ends before
        // the driver first runs still closes the lane.
        let closing = Closing::<K>(Some(lane.clone()), PhantomData);
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

#[path = "anilist_batch_turn.rs"]
mod turn;
use turn::waits_for_turn;
#[cfg(test)]
use turn::{batch_deadline, next_look};
#[path = "anilist_batch_driver.rs"]
mod driver;
use driver::Driver;

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
