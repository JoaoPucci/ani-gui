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
//!   by the show id their value carries, and the ones a page of the
//!   show resolved under another key, found by the page their value
//!   carries, with that key's numbering when no row has the key;
//! - the skip times the player cached for its episodes (`aniskip:`),
//!   found by every Kitsu id the show is known by — the one its row
//!   records, the ones its mapping and title match name, the pages
//!   this process saw it played from — with the rows of the key that
//!   carried the MAL id alone, which nothing reads any more;
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
const HISTORY_PREFIXES: [&str; 5] = [
    "watched-at:",
    "allmanga2kitsu:",
    "title-match:",
    "play:",
    "aniskip:",
];

/// Delete what the row for `id`, titled `title`, recording `recorded`
/// as the show played and seen played from `pages`, left in the cache.
/// Skip times cached under a Kitsu id in `claimed` — one a remaining
/// row records, maps to, matched by title or was seen played from —
/// stay with that row's show. The offsets go after the row
/// ([`sweep_offsets`]). Returns what the removal has to act on next
/// ([`Forgotten`]).
///
/// # Errors
/// Cache write failures propagate.
pub(crate) fn forget_show(
    state: &AppState,
    id: &str,
    title: &str,
    recorded: Option<&str>,
    pages: &[String],
    claimed: &std::collections::HashSet<String>,
) -> Result<Forgotten> {
    let pool = &state.cache_pool;
    meta_cache_delete(pool, &watched_at_key(id))?;
    // Every Kitsu id the player could have asked with for this show:
    // the one the row records, the pages this process saw it played
    // from, and for an older row the ones its mapping and title match
    // named.
    let mut kitsu_ids: Vec<String> = recorded.into_iter().map(str::to_owned).collect();
    kitsu_ids.extend(pages.iter().cloned());
    kitsu_ids.extend(forget_mappings(state, id)?);
    kitsu_ids.extend(super::history_forget_titles::forget_title_matches(
        state, id, title,
    )?);
    // An empty id — a title match stored without one — names no page.
    kitsu_ids.retain(|k| !k.is_empty() && !claimed.contains(k));
    let other_keys = forget_resolutions(state, id, &kitsu_ids)?;
    super::history_forget_skips::forget_skip_times(state, &kitsu_ids)?;
    Ok(Forgotten {
        known_by: kitsu_ids,
        other_keys,
    })
}

/// What forgetting a show found, for the removal to act on.
pub(crate) struct Forgotten {
    /// The Kitsu ids the show was known by that no remaining row
    /// claims — the ones whose skip times and resolution rows went.
    pub(crate) known_by: Vec<String>,
    /// The other show keys those ids' resolution rows named: keys a
    /// page of the show resolved under, another provider's when its
    /// walk failed over.
    pub(crate) other_keys: Vec<String>,
}

/// Delete the show's reverse mapping under every version's key, and
/// return the Kitsu ids they named, expired or not.
fn forget_mappings(state: &AppState, id: &str) -> Result<Vec<String>> {
    let named = super::history_forget_skips::mapping_ids(state, id)?;
    for version in 1..=ALLMANGA_KITSU_VERSION {
        meta_cache_delete(
            &state.cache_pool,
            &format!("allmanga2kitsu:v{version}:{id}"),
        )?;
    }
    Ok(named)
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
/// as the show played, or names one of `known_by` as the page it was
/// resolved from. Returns the other show keys the second kind named.
fn forget_resolutions(state: &AppState, id: &str, known_by: &[String]) -> Result<Vec<String>> {
    let mut other_keys = Vec::new();
    for (key, body) in meta_cache_entries_prefix(&state.cache_pool, "play:")? {
        let row = serde_json::from_str::<serde_json::Value>(&body).unwrap_or_default();
        let field = |name: &str| row.get(name).and_then(|v| v.as_str()).unwrap_or_default();
        let (show, page) = (field("show_id"), field("kitsu_id"));
        let of_the_page = known_by.iter().any(|k| k == page);
        if show != id && !of_the_page {
            continue;
        }
        meta_cache_delete(&state.cache_pool, &key)?;
        if show != id && !show.is_empty() {
            other_keys.push(show.to_owned());
        }
    }
    Ok(other_keys)
}
