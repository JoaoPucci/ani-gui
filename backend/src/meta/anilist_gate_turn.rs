//! Whose turn a request takes ([`super`]): the class it counts as, the
//! rule that a retry stands back while any other request waits, and the
//! line each request sent logs. Split from [`super`] so each file stays
//! inside the CRAP gate's per-file bar.

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::time::Instant;

use super::pace::{Class, Ledger, Pace};

tokio::task_local! {
    static CLASS: Class;
}

/// Run `f` with every AniList request it sends counted as
/// [`Class::Background`] — or as [`Class::Retry`] inside [`as_retry`],
/// which a fill's own background scope does not lift.
pub(crate) async fn in_background<F: Future>(f: F) -> F::Output {
    let class = match current_class() {
        Class::Retry => Class::Retry,
        _ => Class::Background,
    };
    CLASS.scope(class, f).await
}

/// Run `f` with every AniList request it sends counted as
/// [`Class::Retry`].
pub(crate) async fn as_retry<F: Future>(f: F) -> F::Output {
    CLASS.scope(Class::Retry, f).await
}

/// The class a request sent from here counts as.
pub(crate) fn current_class() -> Class {
    CLASS.try_with(|c| *c).unwrap_or(Class::Interactive)
}

/// The earliest a `class` request may leave: the ledger's answer, and
/// for a retry no sooner than a moment from now while a request of
/// another class waits (`fresh_waiting`).
pub(super) fn ready_at(
    pace: &Pace,
    ledger: &Ledger,
    fresh_waiting: &AtomicUsize,
    class: Class,
    now: Instant,
) -> Instant {
    let at = ledger.ready_at(pace, class, now);
    if class == Class::Retry && fresh_waiting.load(Ordering::SeqCst) > 0 {
        return at.max(now + pace.min_gap.max(RETRY_YIELD));
    }
    at
}

/// Counts a request among [`Gate::fresh_waiting`] while it waits.
pub(super) struct Waiting<'a>(&'a AtomicUsize);

impl<'a> Waiting<'a> {
    pub(super) fn new(count: &'a AtomicUsize) -> Self {
        count.fetch_add(1, Ordering::SeqCst);
        Self(count)
    }
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// One debug line per request sent: its class, how full the window it
/// left in is — this send included — against the capacity and the
/// background share, how many of the window's requests were retries,
/// how long it waited for its turn, and whether a retry sent before it
/// made it wait. What the split between the classes is measured from.
pub(super) fn sent(
    pace: &Pace,
    class: Class,
    ledger: &Ledger,
    now: Instant,
    waited: Duration,
    behind_retry: bool,
) {
    tracing::debug!(
        provider = "anilist",
        ?class,
        in_window = ledger.in_window(pace, now),
        retries_in_window = ledger.retries_in_window(pace, now),
        waited_ms = u64::try_from(waited.as_millis()).unwrap_or(u64::MAX),
        behind_retry,
        capacity = pace.capacity,
        background_capacity = pace.capacity_for(Class::Background),
        "anilist request sent"
    );
}

/// How long a retry stands back, at least, while another request waits.
const RETRY_YIELD: Duration = Duration::from_millis(50);
