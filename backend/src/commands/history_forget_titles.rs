//! The title-match rows a history row left, found from its title —
//! split from `history_forget` for the per-file complexity bar. The
//! rule for storing one is `title_match_store`'s.

use crate::app::AppState;
use crate::cache::{meta_cache_delete, meta_cache_entries_prefix};
use crate::commands::kitsu::{title_match_prefix, TITLE_MATCH_VERSION};
use crate::error::Result;
use crate::history::HistoryEntry;
use crate::scraper::provider::ShowKey;
use std::collections::HashSet;

use super::history_title_tail::{all_digits, without_episode_tail};

/// Delete the title-match rows, every version and cour, stored for
/// the row `id` titled `title` — but for those a row in `remaining`
/// searches too. The rows are keyed by what was searched, not by the
/// row: a legacy opaque-id row and a newer slug of one show search the
/// same key, and so does one title on two providers under a version
/// before the provider joined the key. Such a row stays with the row
/// that still searches it.
///
/// # Errors
/// Cache failures propagate.
pub(crate) fn forget_title_matches(
    state: &AppState,
    id: &str,
    title: &str,
    remaining: &[HistoryEntry],
) -> Result<()> {
    let searched: HashSet<String> = remaining
        .iter()
        .flat_map(|e| title_match_prefixes(&e.id, &e.title))
        .collect();
    for prefix in title_match_prefixes(id, title) {
        if searched.contains(&prefix) {
            continue;
        }
        for (key, _) in cour_entries(state, &prefix)? {
            meta_cache_delete(&state.cache_pool, &key)?;
        }
    }
    Ok(())
}

/// The (key, body) pairs of the row `id`'s title-match rows, titled
/// `title`, every version and cour.
fn own_matches(state: &AppState, id: &str, title: &str) -> Result<Vec<(String, String)>> {
    let mut found = Vec::new();
    for prefix in title_match_prefixes(id, title) {
        found.extend(cour_entries(state, &prefix)?);
    }
    Ok(found)
}

/// The key prefixes of the row's title-match rows, every cour to
/// follow. Continue Watching searched the row's title less a legacy
/// "(N episodes)" tail; the current version's key names the provider,
/// the versions before it did not.
fn title_match_prefixes(id: &str, title: &str) -> Vec<String> {
    let provider = ShowKey::parse(id).provider;
    let mut prefixes = Vec::new();
    for searched in [title, without_episode_tail(title)] {
        prefixes.push(title_match_prefix(provider, searched));
        let normalized = searched.trim().to_lowercase();
        for version in 1..TITLE_MATCH_VERSION {
            prefixes.push(format!("title-match:v{version}:{normalized}:c"));
        }
    }
    prefixes
}

/// Delete every title-match row, under any title, that names one of
/// `kitsu_ids`: the ones a removed show was known by and no remaining
/// row claims. A provider can rename a show and the row takes the new
/// title, so rows stored under an earlier title are found by the entry
/// they name, not by the title the row has now. The rows stored for
/// the row `id` under each of `titles` stay, for [`forget_title_matches`]
/// to take afterwards — but for those a remaining row searches too,
/// which it leaves: a retry finds the show's ids by them.
///
/// # Errors
/// Cache failures propagate.
pub(crate) fn forget_title_matches_naming(
    state: &AppState,
    id: &str,
    titles: &[&str],
    kitsu_ids: &[String],
) -> Result<()> {
    let mut own = std::collections::HashSet::new();
    for title in titles {
        own.extend(
            own_matches(state, id, title)?
                .into_iter()
                .map(|(key, _)| key),
        );
    }
    for (key, body) in meta_cache_entries_prefix(&state.cache_pool, "title-match:")? {
        let named = kitsu_ids
            .iter()
            .any(|k| crate::history::same_kitsu_id(k, &body));
        if named && !own.contains(&key) {
            meta_cache_delete(&state.cache_pool, &key)?;
        }
    }
    Ok(())
}

/// The Kitsu ids the title-match rows of the row `id` titled `title`
/// name, every version and cour, left in place.
///
/// # Errors
/// Cache failures propagate.
pub(crate) fn title_match_ids(state: &AppState, id: &str, title: &str) -> Result<Vec<String>> {
    Ok(own_matches(state, id, title)?
        .into_iter()
        .map(|(_, body)| body)
        .collect())
}

/// The (key, body) pairs under `prefix` whose remainder is a cour.
/// The prefix ends where the cour starts, so a title that starts
/// another's ("Re", "Re:Creators") shares it.
fn cour_entries(state: &AppState, prefix: &str) -> Result<Vec<(String, String)>> {
    Ok(meta_cache_entries_prefix(&state.cache_pool, prefix)?
        .into_iter()
        .filter(|(key, _)| all_digits(&key[prefix.len()..]))
        .collect())
}
