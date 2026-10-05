//! A schedule that reaches the airing cache after a show's
//! availability row was written cuts that row the way it would have
//! been cut had the schedule been there first.
//!
//! The write path bounds an ongoing show's row — positive or
//! negative — by the cached next airing, but only when the airing row
//! already exists. Nothing orders the two: the home warm seeds
//! schedules for pre-premiere shows alone, a current show's negative
//! fetches none, a failed seed leaves none, and a detail page asks for
//! availability and airing side by side. So every airing write
//! re-cuts the show's rows here — a local rewrite of their TTL, no
//! request made.

use crate::cache::{meta_cache_row, meta_cache_shorten, SqlitePool};
use crate::commands::availability::{
    cache_key, AvailabilityResponse, AVAILABILITY_TTL_ONGOING_SECS,
};
use crate::commands::availability_ttl::{rescheduled_negative_ttl, rescheduled_ttl};

/// Shorten the show's live availability rows, in both modes, to
/// expire at `next_airing_at` plus the grace the write path gives
/// their kind: an hour for a positive row, three for a negative one.
/// Finished shows' rows, expired rows and rows already expiring
/// sooner are left as they are. Best-effort: a cache error leaves the
/// row its old window.
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
        let Ok(body) = serde_json::from_str::<AvailabilityResponse>(&row.body) else {
            continue;
        };
        let recut = if body.available {
            rescheduled_ttl
        } else {
            rescheduled_negative_ttl
        };
        let age = now.saturating_sub(row.fetched_at);
        // Both kinds are written with the ongoing day when no
        // schedule is known; anything longer is a finished show's
        // window, or a pre-premiere negative a schedule already sized.
        if let Some(ttl) = recut(
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
