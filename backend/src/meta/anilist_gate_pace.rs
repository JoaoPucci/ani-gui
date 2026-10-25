//! The gate's scheduling: who is asking, the pace a host's budget is
//! spent at, and the ledger that schedules against it. Split from
//! [`super`] so each file stays inside the CRAP gate's per-file bar.

use std::collections::VecDeque;
use std::time::Duration;

use tokio::time::Instant;

/// Who is waiting for an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    /// What the user is looking at or did: the default.
    Interactive,
    /// A fill the page renders without: thumbnails, banners, the
    /// airing seeds a warm writes.
    Background,
}

/// How a gate spends its host's budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pace {
    /// The span the capacity is counted over.
    pub window: Duration,
    /// Requests allowed to leave in any one window.
    pub capacity: usize,
    /// Of the capacity, what background fills may not use.
    pub reserve: usize,
    /// The least time between two requests, against a burst limiter.
    pub min_gap: Duration,
    /// The cool-down a refusal without a usable `Retry-After` gets.
    pub default_cool_down: Duration,
    /// How long an interactive caller may wait for its turn.
    pub interactive_wait: Duration,
    /// How long a background caller may wait for its turn.
    pub background_wait: Duration,
}

/// AniList's budget: 25 a minute against its degraded 30, a quarter
/// second apart, five of each minute held for the user. A refusal
/// with no wait is AniList's documented one-minute timeout. An
/// interactive read waits up to three seconds — the retry cap the
/// public transport has always had — and a background fill one: under
/// the two seconds the episode page gives its thumbnail backfill, so a
/// turn is never taken for a request the page has stopped waiting for.
pub(crate) const ANILIST: Pace = Pace {
    window: Duration::from_secs(60),
    capacity: 25,
    reserve: 5,
    min_gap: Duration::from_millis(250),
    default_cool_down: Duration::from_secs(60),
    interactive_wait: Duration::from_secs(3),
    background_wait: Duration::from_secs(1),
};

impl Pace {
    pub(super) fn wait_for(&self, class: Class) -> Duration {
        match class {
            Class::Interactive => self.interactive_wait,
            Class::Background => self.background_wait,
        }
    }

    fn capacity_for(&self, class: Class) -> usize {
        match class {
            Class::Interactive => self.capacity,
            Class::Background => self.capacity.saturating_sub(self.reserve),
        }
        .max(1)
    }
}

/// What a gate remembers: when its recent requests left, and the
/// instant before which nothing may.
#[derive(Debug, Default)]
pub(crate) struct Ledger {
    sent: VecDeque<Instant>,
    not_before: Option<Instant>,
}

impl Ledger {
    /// The earliest instant at or after `now` a request of `class` may
    /// leave: past the cool-down, [`Pace::min_gap`] after the last
    /// request, and late enough that the window ending then holds
    /// fewer requests than the class may send in it.
    pub(crate) fn ready_at(&self, pace: &Pace, class: Class, now: Instant) -> Instant {
        let mut at = now;
        if let Some(not_before) = self.not_before {
            at = at.max(not_before);
        }
        if let Some(&last) = self.sent.back() {
            at = at.max(last + pace.min_gap);
        }
        let cap = pace.capacity_for(class);
        if self.sent.len() >= cap {
            at = at.max(self.sent[self.sent.len() - cap] + pace.window);
        }
        at
    }

    /// A request left at `at`. Only the last `capacity` sends can bound
    /// a later one, so older ones are dropped.
    pub(crate) fn record_send(&mut self, pace: &Pace, at: Instant) {
        self.sent.push_back(at);
        while self.sent.len() > pace.capacity.max(1) {
            self.sent.pop_front();
        }
    }

    /// The request that left at `sent` was answered (or failed) at
    /// `done`: it is counted from then on. One still in flight, or
    /// dropped before its answer, stays counted from when it left. AniList counts a request
    /// when it arrives, which is after it left by however long the
    /// connection took; counting it from its answer keeps the window
    /// this side ends no earlier than the one AniList keeps.
    pub(crate) fn settle(&mut self, sent: Instant, done: Instant) {
        let Some(i) = self.sent.iter().position(|&s| s == sent) else {
            return;
        };
        self.sent.remove(i);
        let at = self.sent.partition_point(|&s| s <= done);
        self.sent.insert(at, done.max(sent));
    }

    /// Nothing may leave before `until`. A cool-down already running
    /// past it stands.
    pub(crate) fn cool_down(&mut self, until: Instant) {
        self.not_before = Some(self.not_before.map_or(until, |nb| nb.max(until)));
    }
}

#[cfg(test)]
#[path = "anilist_gate_prop_test.rs"]
mod prop_tests;
