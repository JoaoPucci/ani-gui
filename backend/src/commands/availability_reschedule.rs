//! A schedule that reaches the airing cache after a show's positive
//! availability row was written cuts that row the way it would have
//! been cut had the schedule been there first.
//!
//! The write path bounds a positive ongoing row by the cached next
//! airing ([`super::availability_ttl::bounded_by_next_airing`]), but
//! only when the airing row already exists. Nothing orders the two:
//! the home warm seeds schedules for pre-premiere shows alone, and a
//! detail page asks for availability and airing side by side. So
//! every airing write re-cuts the show's positive rows here — a local
//! rewrite of their TTL, no request made.

use crate::cache::SqlitePool;

/// Shorten the show's live positive availability rows, in both modes,
/// to expire at `next_airing_at` plus the grace the write path gives.
/// Negative rows, finished shows' rows, expired rows and rows already
/// expiring sooner are left as they are. Best-effort: a cache error
/// leaves the row its old window.
pub(crate) fn shorten_positive_rows(
    _pool: &SqlitePool,
    _kitsu_id: &str,
    _next_airing_at: Option<u64>,
    _now: u64,
) {
}

#[cfg(test)]
#[path = "availability_reschedule_test.rs"]
mod tests;
