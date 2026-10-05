//! What a history row leaves behind, removed with the row.
//!
//! A row records the Kitsu id of the show played. The app keeps more
//! about the show, keyed by the row's show id or title:
//!
//! - the show's watch stamp (`watched-at:`);
//! - its reverse mapping to Kitsu (`allmanga2kitsu:`), under every
//!   version's key;
//! - the title-match rows Continue Watching stored for the row's title
//!   (`title-match:`), under every version's key, and those under any
//!   earlier title that name a Kitsu id the show is known by;
//! - the resolution rows whose stream played the show (`play:`), found
//!   by the show id their value carries, and the ones a page of the
//!   show resolved under another key, found by the page their value
//!   carries — with that key's numbering when no history row has the
//!   key and, for a delete, no other resolution row names it
//!   (`history_forget_resolutions`);
//! - the skip times the player cached for its episodes (`aniskip:`),
//!   found by every Kitsu id the show is known by — the one its row
//!   records, the ones its mapping and title match name, the pages
//!   this process saw it played from — with the rows of the key that
//!   carried the MAL id alone, which nothing reads any more;
//! - the show's numbering offsets, in the file beside the history.
//!   Clearing removes the offsets of the cleared rows' shows and of
//!   the keys their pages resolved under, not those of shows never in
//!   the history.
//!
//! An expired cache row stays on disk until something overwrites it,
//! so expiry is not removal: deleting a row deletes all of these for
//! its show, and clearing the history deletes them for every show.
//! The order is what a retry needs: nothing goes before what the
//! retry would find it by. The watch stamp and skip times; the other
//! keys' numbering, while the resolution rows that name those keys are
//! still there; the resolution rows; the mappings and title matches the
//! show's Kitsu ids are found by; then the history file; then the
//! removed rows' own offsets ([`sweep_offsets`]).

use crate::app::AppState;
use crate::cache::{meta_cache_delete, meta_cache_delete_prefix};
use crate::commands::kitsu::{watched_at_key, ALLMANGA_KITSU_VERSION};
use crate::error::Result;

/// The prefixes every history-derived cache entry lives under.
const HISTORY_PREFIXES: [&str; 6] = [
    "watched-at:",
    "allmanga2kitsu:",
    "title-match:",
    "play:",
    "aniskip:",
    super::kitsu_gone::GONE_PREFIX,
];

/// Delete what the row for `id`, titled `title`, recording `recorded`
/// as the show played and seen played from `pages`, left in the cache,
/// but for what the removal finds things by, which goes after the rest:
/// the resolution rows, which the removal finds with what this returns
/// ([`super::history_forget_resolutions::find_resolutions`]), and then
/// the mapping and title matches this finds the show's Kitsu ids by
/// ([`forget_finders`]). Skip
/// times found by a Kitsu id in `claimed` — one a remaining row
/// records, maps to, matched by title or was seen played from — stay
/// with that row's show. Returns the Kitsu ids the show was known by
/// that no remaining row claims.
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
) -> Result<Vec<String>> {
    let pool = &state.cache_pool;
    meta_cache_delete(pool, &watched_at_key(id))?;
    // Every Kitsu id the player could have asked with for this show:
    // the one the row records, the pages this process saw it played
    // from, and for an older row the ones its mapping and title match
    // named.
    let mut kitsu_ids: Vec<String> = recorded.into_iter().map(str::to_owned).collect();
    kitsu_ids.extend(pages.iter().cloned());
    kitsu_ids.extend(super::history_forget_skips::mapping_ids(state, id)?);
    kitsu_ids.extend(super::history_forget_titles::title_match_ids(
        state, id, title,
    )?);
    // An empty id — a title match stored without one — names no page.
    kitsu_ids.retain(|k| !k.is_empty() && !claimed.contains(k));
    super::history_forget_skips::forget_skip_times(state, &kitsu_ids)?;
    super::kitsu_gone::forget(state, &kitsu_ids)?;
    Ok(kitsu_ids)
}

/// Delete the row `id`'s reverse mapping under every version's key,
/// and the title-match rows stored for it, titled `title`: what
/// [`forget_show`] finds the show's Kitsu ids by, so a removal deletes
/// them once nothing it finds by those ids is left to delete.
///
/// # Errors
/// Cache write failures propagate.
pub(crate) fn forget_finders(state: &AppState, id: &str, title: &str) -> Result<()> {
    for version in 1..=ALLMANGA_KITSU_VERSION {
        meta_cache_delete(
            &state.cache_pool,
            &format!("allmanga2kitsu:v{version}:{id}"),
        )?;
    }
    super::history_forget_titles::forget_title_matches(state, id, title)?;
    Ok(())
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

/// Drop the numbering offsets of the keys in `removed`: the removed
/// rows' own, once the history file no longer holds the rows, and the
/// rowless keys a removed show's page resolved under, before the
/// resolution rows that name them go.
///
/// For a row's own key the order is what keeps the pair whole: a
/// history write that fails leaves the rows with their offsets, and an
/// offsets write that fails leaves offsets without rows, on disk until
/// a later removal or clear names their key. A rowless key's numbering
/// goes before the history
/// write instead, so a retry can still find the key; a removal that
/// then fails has taken it, and a play already under way that writes
/// the key's first row afterwards writes it without its numbering
/// until a resolve stamps it again. A failure here is logged rather
/// than returned, and the removal still succeeds: the numbering stays
/// behind, the row's own as well as a rowless key's, and once the
/// resolution rows go nothing finds a rowless key's again.
///
/// Every other rowless offset stays: a resolve stamps one before the
/// show's first row, and a cached stream played later writes that row
/// through it.
pub(crate) fn sweep_offsets(state: &AppState, removed: &[&str]) {
    let removed = removed.iter().copied().collect();
    if let Err(e) = crate::commands::anidb_offset::forget(state, &removed) {
        tracing::warn!(error = ?e, "offsets removal failed after a history removal");
    }
}
