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

use crate::cache::{meta_cache_row, meta_cache_shorten, SqlitePool};
use crate::commands::availability::{
    cache_key, AvailabilityResponse, AVAILABILITY_TTL_ONGOING_SECS,
};
use crate::commands::availability_ttl::rescheduled_ttl;

/// Shorten the show's live positive availability rows, in both modes,
/// to expire at `next_airing_at` plus the grace the write path gives.
/// Negative rows, finished shows' rows, expired rows and rows already
/// expiring sooner are left as they are. Best-effort: a cache error
/// leaves the row its old window.
pub(crate) fn shorten_positive_rows(
    pool: &SqlitePool,
    kitsu_id: &str,
    next_airing_at: Option<u64>,
    now: u64,
) {
    if next_airing_at.is_none() {
        return;
    }
    for mode in ["sub", "dub"] {
        let key = cache_key(kitsu_id, mode);
        let Ok(Some(row)) = meta_cache_row(pool, &key) else {
            continue;
        };
        let positive = serde_json::from_str::<AvailabilityResponse>(&row.body)
            .is_ok_and(|body| body.available);
        if !positive {
            continue;
        }
        let age = now.saturating_sub(row.fetched_at);
        if let Some(ttl) = rescheduled_ttl(
            row.ttl_seconds,
            age,
            AVAILABILITY_TTL_ONGOING_SECS,
            next_airing_at,
            now,
        ) {
            // Conditional on the row being the one read: a rewrite
            // in between keeps the window its own write gave it.
            let _ = meta_cache_shorten(pool, &key, &row, ttl);
        }
    }
}

#[cfg(test)]
#[path = "availability_reschedule_test.rs"]
mod tests;
