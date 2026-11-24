//! Whether a finished show's catalogue rows have settled: the week a
//! finished show's episode pages and episode stills are kept for
//! ([`crate::cache::ttl::show_row_ttl`]) is earned only once the
//! catalogues have had time to fill in its last episodes.

use crate::cache::{meta_cache_get, SqlitePool};
use crate::meta::kitsu::KitsuAnimeRef;

/// Days past a show's end date before its rows are read as settled.
/// AniList calls a show FINISHED around its last airing and Kitsu from
/// the day after its end date, while the last episodes' stills and
/// titles often come in after that; a week is the same window the
/// detail row itself is kept for.
pub(crate) const SETTLE_DAYS: i64 = 7;

/// Whether a show of `status` that ended on `end_date` (Kitsu's
/// `YYYY-MM-DD`) has settled by `today` (days since 1970-01-01): it is
/// `finished`, and its end date is a real, full date at least
/// [`SETTLE_DAYS`] back. A month-only or missing end says nothing.
#[must_use]
pub(crate) fn settled_finish(status: Option<&str>, end_date: Option<&str>, today: i64) -> bool {
    status == Some("finished")
        && crate::account::episode_map_rules::days(end_date)
            .is_some_and(|ended| today - ended >= SETTLE_DAYS)
}

/// [`settled_finish`] over the show's held detail row, read from the
/// cache only: false when no row is served or it cannot be read.
pub(crate) fn held_settled(pool: &SqlitePool, kitsu_id: &str) -> bool {
    let Some(body) = meta_cache_get(pool, &crate::commands::kitsu::anime_detail_key(kitsu_id))
        .ok()
        .flatten()
    else {
        return false;
    };
    let Ok(detail) = serde_json::from_str::<KitsuAnimeRef>(&body) else {
        return false;
    };
    settled_finish(
        detail.status.as_deref(),
        detail.end_date.as_deref(),
        today(),
    )
}

/// Today, in days since 1970-01-01 (UTC).
fn today() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    i64::try_from(secs / 86_400).unwrap_or(i64::MAX)
}

#[cfg(test)]
#[path = "show_settled_test.rs"]
mod tests;
