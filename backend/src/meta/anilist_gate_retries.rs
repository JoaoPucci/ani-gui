//! The ledger's account of retries ([`super`]): which of its sends were
//! retries, and the turn a request would have had without them. Split
//! from [`super`] so each file stays inside the CRAP gate's per-file bar.

use std::collections::VecDeque;

use tokio::time::Instant;

use super::{Class, Ledger, Pace};

impl Ledger {
    /// A retry left at `at`, besides [`Ledger::record_send`].
    pub(crate) fn record_retry(&mut self, pace: &Pace, at: Instant) {
        self.retries.push_back(at);
        while self.retries.len() > pace.capacity.max(1) {
            self.retries.pop_front();
        }
    }

    /// [`Ledger::ready_at`] as it would be had no retry been sent: a
    /// request for which this is earlier waited behind a retry.
    pub(crate) fn ready_at_without_retries(
        &self,
        pace: &Pace,
        class: Class,
        now: Instant,
    ) -> Instant {
        let mut sent = self.sent.clone();
        for r in &self.retries {
            if let Some(i) = sent.iter().position(|s| s == r) {
                sent.remove(i);
            }
        }
        let other = Ledger {
            sent,
            not_before: self.not_before,
            retries: VecDeque::new(),
        };
        other.ready_at(pace, class, now)
    }

    /// How many retries left less than a window before `now`.
    pub(crate) fn retries_in_window(&self, pace: &Pace, now: Instant) -> usize {
        self.retries
            .iter()
            .filter(|&&s| s + pace.window > now)
            .count()
    }
}
