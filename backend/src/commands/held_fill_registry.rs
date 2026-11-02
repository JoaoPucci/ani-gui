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

#[path = "held_fill_step.rs"]
mod step;
pub(crate) use step::{after_run, Step, MAX_RETRIES};

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
    /// Pending ticket → who waits on it.
    watched: HashMap<u64, Watched>,
}

/// The pages waiting on one pending ticket.
#[derive(Debug, Default)]
struct Watched {
    /// Waits open now that started after the last release.
    pages: usize,
    /// When one of those last started or ended; `None` once a release
    /// let the ticket go and no page has waited on it since.
    seen: Option<Instant>,
    /// Bumped by each release. A wait that started before it no longer
    /// counts, and its end changes nothing.
    era: u64,
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
        self.watched.insert(
            self.last,
            Watched {
                seen: Some(now),
                ..Watched::default()
            },
        );
        Some((self.last, true))
    }

    /// A page started waiting on `ticket`. Returns the release era the
    /// wait belongs to, which its [`Registry::unwatch`] passes back.
    pub(crate) fn watch(&mut self, ticket: u64, now: Instant) -> u64 {
        let Some(w) = self.watched.get_mut(&ticket) else {
            return 0;
        };
        w.pages += 1;
        w.seen = Some(now);
        w.era
    }

    /// A page stopped waiting on `ticket` — its wait answered or gave
    /// out; it may wait again within [`ON_SCREEN_GRACE`]. A wait that
    /// started before the last release (`era` is older) was already let
    /// go, and its end changes nothing.
    pub(crate) fn unwatch(&mut self, ticket: u64, era: u64, now: Instant) {
        if let Some(w) = self.watched.get_mut(&ticket).filter(|w| w.era == era) {
            w.pages = w.pages.saturating_sub(1);
            w.seen = Some(now);
        }
    }

    /// The pages showing `ticket`'s fill have gone: it is off screen at
    /// once — waits still open on it included, which the renderer has
    /// dropped but the backend may hold a while longer — until a page
    /// waits on it again.
    pub(crate) fn release(&mut self, ticket: u64) {
        if let Some(w) = self.watched.get_mut(&ticket) {
            w.pages = 0;
            w.seen = None;
            w.era += 1;
        }
    }

    /// Whether a page shows `ticket`'s fill: one waits on it and has not
    /// let it go since, or did within [`ON_SCREEN_GRACE`].
    pub(crate) fn on_screen(&self, ticket: u64, now: Instant) -> bool {
        self.watched.get(&ticket).is_some_and(|w| {
            w.pages > 0
                || w.seen
                    .is_some_and(|s| now.saturating_duration_since(s) < ON_SCREEN_GRACE)
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
