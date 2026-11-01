//! What the retry of held-back fills remembers ([`super`]): which
//! fills wait for a retry, under which ticket, and how the retries
//! that ended went. Kept apart from the scheduling so the bookkeeping
//! is pure and each file stays inside the complexity bar.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// How many ended tickets are remembered. A page asks about its
/// tickets within seconds of being served them; one asking about a
/// ticket older than this many retries is told it did not land, and
/// keeps what it shows.
pub(crate) const REMEMBERED: usize = 256;

/// Retries a held-back fill gets: one per pause it is held in, up to
/// this many pauses.
pub(crate) const MAX_RETRIES: u32 = 3;

/// The most fills waiting on a retry at once. A fill held back past it
/// gets no retry and no ticket: the next view asks again, as before.
pub(crate) const MAX_PENDING: usize = 128;

/// How long a ticket counts as on screen after the last page waiting on
/// it stopped — the gap between one wait request and the next, and the
/// time a page takes to start waiting after it was served.
pub(crate) const ON_SCREEN_GRACE: Duration = Duration::from_secs(5);

/// Where a ticket stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Standing {
    /// Its retry has not ended.
    Pending,
    /// Its retry ended; `true` when the fill landed in the cache.
    Ended(bool),
}

/// The fills waiting on a retry, by key, and the tickets that ended.
#[derive(Debug, Default)]
pub(crate) struct Registry {
    /// The last ticket issued; tickets count up from 1.
    last: u64,
    /// Key → its pending ticket.
    pending: HashMap<String, u64>,
    /// Ended tickets, oldest first, at most [`REMEMBERED`].
    ended: VecDeque<(u64, bool)>,
    /// Pending ticket → the pages waiting on it now, and when one last
    /// did (`None` once every page let it go).
    watched: HashMap<u64, (usize, Option<Instant>)>,
}

impl Registry {
    /// The ticket a fill held back under `key` waits on, and whether it
    /// is new — a key already waiting keeps its ticket, so one item held
    /// twice is retried once. `None` when [`MAX_PENDING`] fills already
    /// wait. A new ticket counts as on screen from `now`: the page it
    /// was served to is about to wait on it.
    pub(crate) fn hold(&mut self, key: &str, now: Instant) -> Option<(u64, bool)> {
        if let Some(&ticket) = self.pending.get(key) {
            return Some((ticket, false));
        }
        if self.pending.len() >= MAX_PENDING {
            return None;
        }
        self.last += 1;
        self.pending.insert(key.to_owned(), self.last);
        self.watched.insert(self.last, (0, Some(now)));
        Some((self.last, true))
    }

    /// A page started waiting on `ticket`.
    pub(crate) fn watch(&mut self, ticket: u64, now: Instant) {
        if let Some((pages, seen)) = self.watched.get_mut(&ticket) {
            *pages += 1;
            *seen = Some(now);
        }
    }

    /// A page stopped waiting on `ticket` — its wait answered or gave
    /// out; it may wait again within [`ON_SCREEN_GRACE`].
    pub(crate) fn unwatch(&mut self, ticket: u64, now: Instant) {
        if let Some((pages, seen)) = self.watched.get_mut(&ticket) {
            *pages = pages.saturating_sub(1);
            if seen.is_some() {
                *seen = Some(now);
            }
        }
    }

    /// The pages showing `ticket`'s fill have gone: it is off screen at
    /// once, until a page waits on it again.
    pub(crate) fn release(&mut self, ticket: u64) {
        if let Some((_, seen)) = self.watched.get_mut(&ticket) {
            *seen = None;
        }
    }

    /// Whether a page shows `ticket`'s fill: one waits on it, or did
    /// within [`ON_SCREEN_GRACE`] and did not let it go.
    pub(crate) fn on_screen(&self, ticket: u64, now: Instant) -> bool {
        self.watched.get(&ticket).is_some_and(|&(pages, seen)| {
            pages > 0 || seen.is_some_and(|s| now.saturating_duration_since(s) < ON_SCREEN_GRACE)
        })
    }

    /// Whether `ticket`'s retry may take its turn now, by rank: a fill
    /// on screen may; one no page shows any more only while no fill on
    /// screen waits for a retry.
    pub(crate) fn may_go(&self, ticket: u64, now: Instant) -> bool {
        self.on_screen(ticket, now)
            || !self
                .pending
                .values()
                .any(|&t| t != ticket && self.on_screen(t, now))
    }

    /// The ticket `key` waits on, if it waits.
    pub(crate) fn pending(&self, key: &str) -> Option<u64> {
        self.pending.get(key).copied()
    }

    /// `ticket`'s retry under `key` ended, landed or not. The key is free
    /// again: a fill held after this is a new item.
    pub(crate) fn end(&mut self, key: &str, ticket: u64, landed: bool) {
        if self.pending.get(key) != Some(&ticket) {
            return;
        }
        self.pending.remove(key);
        self.watched.remove(&ticket);
        self.ended.push_back((ticket, landed));
        while self.ended.len() > REMEMBERED {
            self.ended.pop_front();
        }
    }

    /// Where `ticket` stands. One never issued, or ended too long ago to
    /// be remembered, did not land.
    pub(crate) fn standing(&self, ticket: u64) -> Standing {
        if self.pending.values().any(|&t| t == ticket) {
            return Standing::Pending;
        }
        let landed = self
            .ended
            .iter()
            .find(|&&(t, _)| t == ticket)
            .is_some_and(|&(_, landed)| landed);
        Standing::Ended(landed)
    }
}

/// What a retry does after a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// The fill is in the cache.
    Landed,
    /// Held back again: wait out this pause too and run once more.
    Again,
    /// Not landed, and no pause to wait out — a failure that is not a
    /// rate limit — or out of retries.
    GiveUp,
}

/// The step after run number `attempt` (from 1): `landed` when the fill
/// is now in the cache, `paused` when the gate is holding background
/// fills back.
pub(crate) fn after_run(landed: bool, paused: bool, attempt: u32) -> Step {
    if landed {
        Step::Landed
    } else if paused && attempt < MAX_RETRIES {
        Step::Again
    } else {
        Step::GiveUp
    }
}
