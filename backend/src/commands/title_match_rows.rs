//! A history row's title-match rows, read from its title: the key
//! prefixes every version wrote them under, and the Kitsu ids they
//! name. Split from `history_forget_titles` for the per-file
//! complexity bar.

use crate::app::AppState;
use crate::cache::meta_cache_entries_prefix;
use crate::commands::kitsu::{
    title_match_prefix, title_match_prefix_at, FIRST_SEGMENTED_TITLE_MATCH_VERSION,
    TITLE_MATCH_VERSION,
};
use crate::commands::title_match_store::retired_title_match_prefixes;
use crate::error::Result;
use crate::scraper::provider::ShowKey;

use super::history_title_tail::{all_digits, without_episode_tail};

/// The (key, body) pairs of the row `id`'s title-match rows, titled
/// `title`, every version and cour.
pub(crate) fn own_matches(
    state: &AppState,
    id: &str,
    title: &str,
) -> Result<Vec<(String, String)>> {
    let mut found = Vec::new();
    for prefix in title_match_prefixes(id, title) {
        found.extend(cour_entries(state, &prefix)?);
    }
    Ok(found)
}

/// The key prefixes of the row's title-match rows, every cour to
/// follow. Continue Watching searched the row's title less a legacy
/// "(N episodes)" tail; the current version's key names the provider,
/// the versions before it did not. A bare row's matches also sit under
/// the segments retired with the provider whose rows were bare
/// ([`retired_title_match_prefixes`]).
pub(crate) fn title_match_prefixes(id: &str, title: &str) -> Vec<String> {
    let provider = ShowKey::parse(id).provider;
    let mut prefixes = current_prefixes(id, title);
    for searched in [title, without_episode_tail(title)] {
        // The versions since the provider segment, each as it wrote it,
        // and the ones before it, which keyed the title alone.
        for version in FIRST_SEGMENTED_TITLE_MATCH_VERSION..TITLE_MATCH_VERSION {
            prefixes.push(title_match_prefix_at(version, provider, searched));
        }
        let normalized = searched.trim().to_lowercase();
        prefixes.extend(retired_title_match_prefixes(provider, &normalized));
        for version in 1..FIRST_SEGMENTED_TITLE_MATCH_VERSION {
            prefixes.push(format!("title-match:v{version}:{normalized}:c"));
        }
    }
    prefixes
}

/// The current version's key prefixes of the row's title matches: its
/// title as listed and less a legacy episode tail, each as it is and
/// keyed by the year the row names — its title's, else its key's
/// ([`crate::commands::title_match_year`]).
fn current_prefixes(id: &str, title: &str) -> Vec<String> {
    let key = ShowKey::parse(id);
    let year = crate::commands::title_match_year::title_year(title)
        .or_else(|| crate::scraper::provider::key_year(id));
    let mut prefixes = Vec::new();
    for searched in [title, without_episode_tail(title)] {
        prefixes.push(title_match_prefix(key.provider, searched));
        let keyed = crate::commands::title_match_year::keyed_title(searched, year);
        if keyed != searched {
            prefixes.push(title_match_prefix(key.provider, &keyed));
        }
    }
    prefixes
}

/// The Kitsu ids the row's title matches name under the current
/// version's key alone — the ones Continue Watching reads now. A match
/// an earlier version stored is a pick that version made, which Home
/// no longer reads, so the detail page's resume lookup does not read
/// it either; a removal still does, to find every id the show was
/// known by.
///
/// # Errors
/// Cache failures propagate.
pub(crate) fn current_title_match_ids(
    state: &AppState,
    id: &str,
    title: &str,
) -> Result<Vec<String>> {
    let mut found = Vec::new();
    for prefix in current_prefixes(id, title) {
        found.extend(
            cour_entries(state, &prefix)?
                .into_iter()
                .map(|(_, body)| body),
        );
    }
    Ok(found)
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
pub(crate) fn cour_entries(state: &AppState, prefix: &str) -> Result<Vec<(String, String)>> {
    Ok(meta_cache_entries_prefix(&state.cache_pool, prefix)?
        .into_iter()
        .filter(|(key, _)| all_digits(&key[prefix.len()..]))
        .collect())
}
