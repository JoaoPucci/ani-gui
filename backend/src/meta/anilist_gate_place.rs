//! A lane's place in a gate's queue ([`super`]): how long until a
//! request of a class could take its turn, and a place that counts as a
//! request waiting for its turn while a lane holds lookups for that
//! turn rather than sending them into the gate one by one
//! ([`crate::meta::anilist_batch`]). Split from [`super`] so each file
//! stays inside the CRAP gate's per-file bar.

use std::cell::RefCell;
use std::future::Future;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use super::{gate_for, Class, Gate};

/// How long until a request of `class` could take its turn through
/// `url`'s gate: zero when it could now, `None` when the turn comes
/// past the class's wait and the gate would turn it away.
pub(crate) fn turn_in(url: &str, class: Class) -> Option<Duration> {
    let gate = gate_for(url);
    let now = Instant::now();
    let at = {
        let ledger = gate.ledger();
        gate.ready_at(&ledger, class, now)
    };
    within(at.saturating_duration_since(now), gate.pace.wait_for(class))
}

/// `wait`, when a caller allowed `budget` would wait for it.
pub(crate) fn within(wait: Duration, budget: Duration) -> Option<Duration> {
    (wait <= budget).then_some(wait)
}

/// Counts, while it lives, as a request other than a retry waiting for
/// its turn at a gate: no retry takes a turn ahead of the lookups a
/// lane holds for one.
pub(crate) struct Place(Arc<Gate>);

/// A place at `url`'s gate.
pub(crate) fn place(url: &str) -> Place {
    let gate = gate_for(url);
    gate.fresh_waiting.fetch_add(1, Ordering::SeqCst);
    Place(gate)
}

impl Drop for Place {
    fn drop(&mut self) {
        self.0.fresh_waiting.fetch_sub(1, Ordering::SeqCst);
    }
}

tokio::task_local! {
    /// The place the requests sent here hold until the first of them
    /// takes its turn.
    static HANDED: RefCell<Option<Place>>;
}

/// Run `f`, keeping `place` until the first request it sends takes its
/// turn, or until `f` ends: the lane's lookups handed to the gate keep
/// the place they held in the lane, with no moment between in which a
/// retry could take the turn.
pub(crate) async fn handing<F: Future>(place: Option<Place>, f: F) -> F::Output {
    HANDED.scope(RefCell::new(place), f).await
}

/// A request sent here has taken its turn: the place it was handed is
/// given up, in the same step.
pub(super) fn turn_taken() {
    let _ = HANDED.try_with(|place| place.borrow_mut().take());
}

#[cfg(test)]
#[path = "anilist_gate_place_test.rs"]
mod tests;
