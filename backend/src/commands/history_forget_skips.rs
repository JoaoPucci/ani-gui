//! The skip times a history row's show left, found by Kitsu id —
//! split from `history_forget` for the per-file complexity bar.

use std::collections::HashSet;

use crate::app::AppState;
use crate::cache::{meta_cache_delete_prefix, meta_cache_entries_prefix};
use crate::commands::kitsu::ALLMANGA_KITSU_VERSION;
use crate::error::Result;
use crate::history::HistoryEntry;

/// Every Kitsu id the `remaining` rows claim: the one each records,
/// and the ones its mapping and title match name. Skip times cached
/// under these stay when another row is removed.
///
/// # Errors
/// Cache failures propagate.
pub(crate) fn claimed_ids(state: &AppState, remaining: &[HistoryEntry]) -> Result<HashSet<String>> {
    let mut claimed = HashSet::new();
    for entry in remaining {
        claimed.extend(entry.kitsu_id.clone());
        claimed.extend(mapping_ids(state, &entry.id)?);
        claimed.extend(super::history_forget_titles::title_match_ids(
            state,
            &entry.id,
            &entry.title,
        )?);
    }
    Ok(claimed)
}

/// The Kitsu ids the show's reverse mapping names under every
/// version's key, expired or not.
///
/// # Errors
/// Cache failures propagate.
pub(crate) fn mapping_ids(state: &AppState, id: &str) -> Result<Vec<String>> {
    let mut named = Vec::new();
    for version in 1..=ALLMANGA_KITSU_VERSION {
        let key = format!("allmanga2kitsu:v{version}:{id}");
        named.extend(
            meta_cache_entries_prefix(&state.cache_pool, &key)?
                .into_iter()
                .filter(|(found, _)| *found == key)
                .map(|(_, body)| body),
        );
    }
    Ok(named)
}

/// Delete the skip times cached under each of `kitsu_ids`, and every
/// row of the key that carried the MAL id alone, which nothing reads
/// any more. A Kitsu id is digits, so the colon after it keeps one id
/// from matching another it starts.
///
/// # Errors
/// Cache write failures propagate.
pub(crate) fn forget_skip_times(state: &AppState, kitsu_ids: &[String]) -> Result<()> {
    for id in kitsu_ids.iter().filter(|id| !id.is_empty()) {
        meta_cache_delete_prefix(&state.cache_pool, &format!("aniskip:v2:{id}:"))?;
    }
    meta_cache_delete_prefix(&state.cache_pool, "aniskip:v1:")
}
