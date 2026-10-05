//! Kitsu ids Kitsu has answered are gone. Kitsu can delete an entry,
//! and a history row that recorded its id would otherwise go on naming
//! it: the detail page's resume lookup matches a row's recorded id
//! before anything else. The mark is written only by a detail fetch
//! something already asked for — the home page reading a row's id —
//! and only for a 404 or 410; every other failure says nothing about
//! the id. A later fetch that succeeds clears it.

use crate::app::AppState;
use crate::cache::{meta_cache_delete, meta_cache_get, meta_cache_put};
use crate::error::{AniError, Result};

/// How long Kitsu's answer stands without another fetch of the id.
/// Long: the mark only matters while a history row names the id, and
/// a fetch that succeeds clears it whenever it lands.
const GONE_TTL_SECS: u64 = 365 * 24 * 60 * 60;

fn gone_key(id: &str) -> String {
    format!("kitsu:dead:{id}")
}

/// Remember `id` as gone when `err` is Kitsu answering so. A cache
/// that cannot store the mark leaves the id standing, the state before
/// the mark existed.
pub(crate) fn note_failure(state: &AppState, id: &str, err: &AniError) {
    if err.is_not_found_shaped() {
        let _ = meta_cache_put(&state.cache_pool, &gone_key(id), "1", GONE_TTL_SECS);
    }
}

/// Kitsu served `id`: it is not gone, whatever an earlier answer said.
pub(crate) fn note_served(state: &AppState, id: &str) {
    let _ = meta_cache_delete(&state.cache_pool, &gone_key(id));
}

/// Whether Kitsu last answered that `id` is gone.
///
/// # Errors
/// SQLite read failures propagate.
pub(crate) fn is_gone(state: &AppState, id: &str) -> Result<bool> {
    Ok(meta_cache_get(&state.cache_pool, &gone_key(id))?.is_some())
}
