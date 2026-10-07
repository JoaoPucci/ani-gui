//! The hold an unpaced download puts on every host's budget: while it
//! runs, every host's tokens are held to what its requests in flight
//! leave of the burst, and when it ends every host, and one not yet
//! fetched from, is left where the hold left it.

use std::time::Duration;

use tokio::time::Instant;

use super::{Bucket, HostBudget};

/// Tops the bucket up at `now`, as a take does, and holds it to `most`
/// tokens.
fn hold_to(bucket: &mut Bucket, now: Instant, burst: u32, refill: Duration, most: u32) {
    let elapsed = now.saturating_duration_since(bucket.refilled_at);
    let refilled = elapsed.as_secs_f64() / refill.as_secs_f64();
    bucket.tokens = (bucket.tokens + refilled)
        .min(f64::from(burst))
        .min(f64::from(most));
    bucket.refilled_at = now;
}

/// The unpaced downloads running — the requests they may have in
/// flight between them — and, once one has ended, where that left a
/// host the player has not fetched from yet: a bucket held as every
/// host's was, refilling as theirs do.
#[derive(Debug, Default)]
pub(super) struct Unpaced {
    in_flight: u32,
    untouched: Option<Bucket>,
}

/// An unpaced download running: while it runs, every host's tokens are
/// held to what its requests in flight leave of the burst — its
/// fragments go wherever its playlists send them, which the app does
/// not see — and when it ends every host is left where the hold left
/// it, a host not yet fetched from starting there too.
pub(crate) struct UnpacedRun<'a> {
    budget: &'a HostBudget,
    in_flight: u32,
}

impl Drop for UnpacedRun<'_> {
    fn drop(&mut self) {
        let now = Instant::now();
        // Hosts first, then the run count, the order admissions take
        // them in: no admission sees the run ended before every bucket
        // is held.
        let mut states = self.budget.states.lock().unwrap_or_else(|e| e.into_inner());
        // Every host was held to what all the runs going leave, this
        // one's requests and any other's, so that is where it ends.
        let most = {
            let mut unpaced = self
                .budget
                .unpaced
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let most = self.budget.held_to(unpaced.in_flight);
            unpaced.in_flight = unpaced.in_flight.saturating_sub(self.in_flight);
            let untouched = unpaced
                .untouched
                .get_or_insert_with(|| Bucket::full(self.budget.burst, now));
            hold_to(untouched, now, self.budget.burst, self.budget.refill, most);
            most
        };
        for state in states.values_mut() {
            hold_to(
                &mut state.bucket,
                now,
                self.budget.burst,
                self.budget.refill,
                most,
            );
        }
    }
}

impl HostBudget {
    /// An unpaced download starts, with up to `in_flight` requests at a
    /// time that pass the budget: every host's tokens are held to what
    /// they leave of the burst until it ends ([`UnpacedRun`]).
    pub(crate) fn unpaced_run(&self, in_flight: u32) -> UnpacedRun<'_> {
        self.unpaced
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .in_flight += in_flight;
        UnpacedRun {
            budget: self,
            in_flight,
        }
    }

    /// The tokens a host is held to beside `in_flight` requests that
    /// pass the budget: what they leave of the burst, never below one,
    /// so the hold alone never empties a bucket.
    fn held_to(&self, in_flight: u32) -> u32 {
        self.burst.saturating_sub(in_flight).max(1)
    }

    /// A bucket for a host first fetched from at `now`: full, or, once
    /// an unpaced download has ended, where the holds left every host,
    /// refilling from then.
    pub(super) fn new_bucket(&self, now: Instant) -> Bucket {
        self.unpaced
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .untouched
            .unwrap_or_else(|| Bucket::full(self.burst, now))
    }

    /// The reserve background traffic leaves while unpaced downloads
    /// run: no more than what the hold leaves, less the token being
    /// taken, so background traffic is not shut out for as long as a
    /// download runs free.
    pub(super) fn reserve_under_hold(&self, reserve: u32) -> u32 {
        let in_flight = self
            .unpaced
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .in_flight;
        if in_flight == 0 {
            return reserve;
        }
        reserve.min(self.held_to(in_flight) - 1)
    }

    /// While unpaced downloads run, holds a host's tokens to what their
    /// requests in flight leave of the burst ([`Self::held_to`]).
    pub(super) fn hold_to_unpaced(&self, bucket: &mut Bucket, now: Instant) {
        let in_flight = self
            .unpaced
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .in_flight;
        if in_flight > 0 {
            hold_to(
                bucket,
                now,
                self.burst,
                self.refill,
                self.held_to(in_flight),
            );
        }
    }
}
