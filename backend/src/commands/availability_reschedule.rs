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
use crate::commands::availability::{cache_key, AvailabilityResponse};
use crate::commands::availability_ttl::{
    next_airing_for_count, rescheduled_negative_ttl, rescheduled_ttl,
};

/// Shorten the show's live availability rows, in both modes, to
/// expire at `next_airing_at` plus the grace the write path gives
/// their kind: an hour for a positive row, three for a negative one.
/// A positive row whose count `aired` has passed is cut as though the
/// airing were now ([`next_airing_for_count`]). Expired rows and rows
/// already expiring sooner are left as they are, and a finished show
/// has no next airing to cut at. Best-effort: a cache error leaves
/// the row its old window.
pub(crate) fn cut_rows_at_next_airing(
    pool: &SqlitePool,
    kitsu_id: &str,
    next_airing_at: Option<u64>,
    aired: Option<u32>,
    now: u64,
) {
    for mode in ["sub", "dub"] {
        let key = cache_key(kitsu_id, mode);
        let Ok(Some(row)) = meta_cache_row(pool, &key) else {
            continue;
        };
        let Ok(body) = serde_json::from_str::<AvailabilityResponse>(&row.body) else {
            continue;
        };
        let age = now.saturating_sub(row.fetched_at);
        let cut = if body.available {
            // A count the schedule has passed is cut as at a drop. Only
            // while a next airing is scheduled: with none, the show may
            // be finished, whose row a lagging count must not churn.
            let at = next_airing_at.and_then(|_| {
                next_airing_for_count(next_airing_at, body.episode_count, aired, now)
            });
            rescheduled_ttl(row.ttl_seconds, age, at, now)
        } else {
            rescheduled_negative_ttl(row.ttl_seconds, age, next_airing_at, now)
        };
        if let Some(ttl) = cut {
            // Conditional on the row being the one read: a rewrite
            // in between keeps the window its own write gave it.
            let _ = meta_cache_shorten(pool, &key, &row, ttl);
        }
    }
}

#[cfg(test)]
#[path = "availability_reschedule_test.rs"]
mod tests;
