//! What a history row leaves behind, removed with the row.
//!
//! A row records the Kitsu id of the show played. The app keeps more
//! about the show, keyed by the row's show id or title:
//!
//! - the show's watch stamp (`watched-at:`);
//! - its reverse mapping to Kitsu (`allmanga2kitsu:`), under every
//!   version's key;
//! - the title-match rows Continue Watching stored for the row's title
//!   (`title-match:`), under every version's key;
//! - the resolution rows whose stream played the show (`play:`), found
//!   by the show id their value carries;
//! - the show's numbering offsets, in the file beside the history.
//!   Clearing removes the offsets of the cleared rows' shows, not those
//!   of shows a resolve stamped before they had a row.
//!
//! An expired cache row stays on disk until something overwrites it,
//! so expiry is not removal: deleting a row deletes all of these for
//! its show, and clearing the history deletes them for every show. The
//! cache entries go before the history file is rewritten and the
//! offsets after it ([`sweep_offsets`]).

use crate::app::AppState;
use crate::cache::{meta_cache_delete, meta_cache_delete_prefix, meta_cache_entries_prefix};
use crate::commands::kitsu::{watched_at_key, ALLMANGA_KITSU_VERSION};
use crate::error::Result;

/// The prefixes every history-derived cache entry lives under.
const HISTORY_PREFIXES: [&str; 4] = ["watched-at:", "allmanga2kitsu:", "title-match:", "play:"];

/// Delete what the row for `id`, titled `title`, left in the cache. The
/// offsets go after the row ([`sweep_offsets`]).
///
/// # Errors
/// Cache write failures propagate.
pub(crate) fn forget_show(state: &AppState, id: &str, title: &str) -> Result<()> {
    let pool = &state.cache_pool;
    meta_cache_delete(pool, &watched_at_key(id))?;
    for version in 1..=ALLMANGA_KITSU_VERSION {
        meta_cache_delete(pool, &format!("allmanga2kitsu:v{version}:{id}"))?;
    }
    super::history_forget_titles::forget_title_matches(state, id, title)?;
    forget_resolutions(state, id)
}

/// Delete everything every history row left in the cache. The offsets
/// go after the rows ([`sweep_offsets`]).
///
/// # Errors
/// Cache write failures propagate.
pub(crate) fn forget_all(state: &AppState) -> Result<()> {
    for prefix in HISTORY_PREFIXES {
        meta_cache_delete_prefix(&state.cache_pool, prefix)?;
    }
    Ok(())
}

/// Drop the numbering offsets of the removed rows' shows, once the
/// history file no longer holds them. The order is what keeps the pair
/// whole: a history write that fails leaves the rows with their
/// offsets, and an offsets write that fails leaves only offsets without
/// rows, which nothing reads. A failure here is logged rather than
/// returned, since the rows the caller removed are gone. Offsets of
/// shows that never had a row stay: a resolve stamps one before the
/// show's first row, which a cache-hit play then writes through it.
pub(crate) fn sweep_offsets(state: &AppState, removed: &[&str]) {
    let removed = removed.iter().copied().collect();
    if let Err(e) = crate::commands::anidb_offset::forget(state, &removed) {
        tracing::warn!(error = ?e, "offsets removal failed after a history removal");
    }
}

/// Delete the resolution rows, of any schema, whose value names `id`
/// as the show played.
fn forget_resolutions(state: &AppState, id: &str) -> Result<()> {
    for (key, body) in meta_cache_entries_prefix(&state.cache_pool, "play:")? {
        let show = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("show_id")?.as_str().map(str::to_owned));
        if show.as_deref() == Some(id) {
            meta_cache_delete(&state.cache_pool, &key)?;
        }
    }
    Ok(())
}
