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

#[path = "kitsu_gone_served.rs"]
mod served;
#[cfg(test)]
pub(crate) use served::note_served_then;
pub(crate) use served::{publish_served, served_from_cache, served_now_then};

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
    // A mark is kept under the Kitsu id a history row could record,
    // and only for a read of exactly that id. The detail read trims
    // what it is given before asking Kitsu (`crate::kitsu_id::require`)
    // and passes the digits it asked for; a caller passing anything
    // else, a padded value included, did not ask Kitsu for those
    // digits, so its 404 says nothing about them and marks nothing.
    if crate::history::kitsu_id_in(id) != Some(id) {
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
