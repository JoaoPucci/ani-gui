//! Kitsu ids Kitsu has answered are gone. Kitsu can delete an entry,
//! and a history row that recorded its id would otherwise go on naming
//! it: the detail page's resume lookup matches a row's recorded id
//! before anything else. The mark is written by any detail fetch a
//! user action makes — never by a warm or background path — and only
//! for a 404 or 410, and takes the id's cached detail row with it;
//! every other failure says nothing about the id. A later
//! fetch that succeeds clears it, as does a detail served from the
//! cache, and neither verdict outlives a newer one: a failure of a read
//! begun before Kitsu last served the id marks nothing, and a success
//! is published — its row cached, the mark taken — in one step with
//! its verdict, and not at all once a read begun after Kitsu served it
//! has been answered gone: that read then reports the id gone too. A
//! mark that cannot be taken fails the read.
//! Removing history takes it ([`super::history_forget`]): a clear takes every mark, a delete the
//! marks of the ids the show was known by that no remaining row claims.
//! A read begun before such a removal writes no mark after it
//! ([`crate::history::guard`]); a clear after one only takes data, so
//! it needs no guard.

use crate::app::AppState;
use crate::cache::{meta_cache_delete, meta_cache_get, meta_cache_put};
use crate::error::{AniError, Result};
use crate::history::guard::Epoch;

/// How long Kitsu's answer stands without another fetch of the id.
/// Long: the mark only matters while a history row names the id, and
/// a fetch that succeeds clears it whenever it lands.
const GONE_TTL_SECS: u64 = 365 * 24 * 60 * 60;

/// The key every mark starts with.
pub(crate) const GONE_PREFIX: &str = "kitsu:dead:";

fn gone_key(id: &str) -> String {
    format!("{GONE_PREFIX}{id}")
}

/// Remember `id` as gone when `err` is Kitsu answering so to a read
/// begun at `begun` — unless a show known by `id` was removed from
/// history, or the history cleared, since: the removal took the id's
/// mark, and this one would bring it back. The id's cached detail row
/// goes with the mark, in the same step: a row beside the mark would
/// answer the next read as Kitsu serving the id. A cache that cannot
/// delete the row or store the mark leaves the id standing, the state
/// before the mark existed.
pub(crate) fn note_failure(state: &AppState, begun: Epoch, id: &str, err: &AniError) {
    if !err.is_not_found_shaped() {
        return;
    }
    crate::history::guard::hold(&state.history_path, |held| {
        if held.kitsu_removed_since(begun, id) || held.kitsu_served_since(begun, id) {
            return;
        }
        let detail = super::kitsu::anime_detail_key(id);
        if meta_cache_delete(&state.cache_pool, &detail).is_ok()
            && meta_cache_put(&state.cache_pool, &gone_key(id), "1", GONE_TTL_SECS).is_ok()
        {
            held.kitsu_gone(id);
        }
    });
}

/// Kitsu served `id` to a read, now: the moment the read's
/// [`publish_served`] is judged by. Nothing is published yet — the
/// read still has its detail row to finish and cache.
pub(crate) fn note_fetched(state: &AppState, id: &str) -> Epoch {
    crate::history::guard::hold(&state.history_path, |held| held.kitsu_served(id))
}

/// Publish what a read Kitsu served at `fetched` found: take the id's
/// mark and run `cache` (the detail row's write), as one step —
/// unless Kitsu has since answered a newer read that the id is gone.
/// That answer stands: the older success caches nothing over it and
/// reports it.
///
/// # Errors
/// - A not-found error when a newer read was answered gone: the read
///   answers as the mark does.
/// - A mark that cannot be deleted: the id is not reported served
///   while its mark still says it is gone, and no row is cached
///   beside it.
pub(crate) fn publish_served(
    state: &AppState,
    id: &str,
    fetched: Epoch,
    cache: impl FnOnce(),
) -> Result<()> {
    crate::history::guard::hold(&state.history_path, |held| {
        if held.kitsu_gone_since(fetched, id) {
            return Err(AniError::Upstream { status: 404 });
        }
        take_mark(state, id)?;
        cache();
        Ok(())
    })
}

/// A detail served from the cache under `key`: read the row, and when
/// `parse` takes it, note Kitsu serving `id` and take its mark — one
/// step, so a 404 noted between the read and the taking cannot be the
/// mark taken. `None` when there is no row `parse` takes.
///
/// # Errors
/// SQLite read failures, and a mark that cannot be deleted.
pub(crate) fn served_from_cache<T>(
    state: &AppState,
    id: &str,
    key: &str,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Result<Option<(T, String)>> {
    crate::history::guard::hold(&state.history_path, |held| {
        let Some(body) = meta_cache_get(&state.cache_pool, key)? else {
            return Ok(None);
        };
        let Some(value) = parse(&body) else {
            return Ok(None);
        };
        held.kitsu_served(id);
        take_mark(state, id)?;
        Ok(Some((value, body)))
    })
}

/// Kitsu served `id` now, to a lookup that has the detail in hand:
/// note it, take the id's mark and run `cache` as one step, then run
/// `then` — the point a test fails another read of `id` at.
///
/// The served moment and the mark go in one hold: a read begun after
/// the moment is newer than this answer, and the mark its failure
/// writes must not be the one taken here.
///
/// # Errors
/// A mark that cannot be deleted; nothing is cached then.
pub(crate) fn served_now_then(
    state: &AppState,
    id: &str,
    cache: impl FnOnce(),
    then: impl FnOnce(&AppState),
) -> Result<()> {
    crate::history::guard::hold(&state.history_path, |held| -> Result<()> {
        held.kitsu_served(id);
        take_mark(state, id)?;
        cache();
        Ok(())
    })?;
    then(state);
    Ok(())
}

/// [`served_now_then`] with nothing cached — the seam a test notes a
/// success through.
#[cfg(test)]
pub(crate) fn note_served_then(state: &AppState, id: &str, served: impl FnOnce(&AppState)) {
    served_now_then(state, id, || {}, served).expect("mark taken");
}

/// Delete `id`'s mark when there is one, or when whether there is one
/// cannot be read.
fn take_mark(state: &AppState, id: &str) -> Result<()> {
    if is_gone(state, id).unwrap_or(true) {
        meta_cache_delete(&state.cache_pool, &gone_key(id))?;
    }
    Ok(())
}

/// Whether Kitsu last answered that `id` is gone.
///
/// # Errors
/// SQLite read failures propagate.
pub(crate) fn is_gone(state: &AppState, id: &str) -> Result<bool> {
    Ok(meta_cache_get(&state.cache_pool, &gone_key(id))?.is_some())
}

/// Delete the marks of `kitsu_ids`.
///
/// # Errors
/// SQLite write failures propagate.
pub(crate) fn forget(state: &AppState, kitsu_ids: &[String]) -> Result<()> {
    for id in kitsu_ids {
        meta_cache_delete(&state.cache_pool, &gone_key(id))?;
    }
    Ok(())
}
