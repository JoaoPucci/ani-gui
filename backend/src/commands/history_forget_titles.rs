//! The title-match rows a history row left, found from its title —
//! split from `history_forget` for the per-file complexity bar. The
//! rule for storing one is `title_match_store`'s.

use crate::app::AppState;
use crate::cache::{meta_cache_delete, meta_cache_entries_prefix};
use crate::commands::kitsu::{title_match_prefix, TITLE_MATCH_VERSION};
use crate::error::Result;
use crate::scraper::provider::ShowKey;

/// Delete the title-match rows, every version and cour, stored for
/// the row `id` titled `title`.
///
/// # Errors
/// Cache failures propagate.
pub(crate) fn forget_title_matches(state: &AppState, id: &str, title: &str) -> Result<()> {
    for prefix in title_match_prefixes(id, title) {
        for (key, _) in cour_entries(state, &prefix)? {
            meta_cache_delete(&state.cache_pool, &key)?;
        }
    }
    Ok(())
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
/// they name, not by the title the row has now.
///
/// # Errors
/// Cache failures propagate.
pub(crate) fn forget_title_matches_naming(state: &AppState, kitsu_ids: &[String]) -> Result<()> {
    for (key, body) in meta_cache_entries_prefix(&state.cache_pool, "title-match:")? {
        if kitsu_ids.contains(&body) {
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
    let mut named = Vec::new();
    for prefix in title_match_prefixes(id, title) {
        named.extend(
            cour_entries(state, &prefix)?
                .into_iter()
                .map(|(_, body)| body),
        );
    }
    Ok(named)
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

/// `title` less a trailing `(N episodes)`, itself optionally followed
/// by `(year)` — the tail rows written before the provider migration
/// carry, which Continue Watching strips before it searches.
pub(crate) fn without_episode_tail(title: &str) -> &str {
    let is_year = |inner: &str| (1..=4).contains(&inner.len()) && all_digits(inner);
    let before_year = strip_paren_tail(title, is_year).unwrap_or(title);
    strip_paren_tail(before_year, is_episode_count)
        .or_else(|| strip_paren_tail(title, is_episode_count))
        .unwrap_or(title)
        .trim()
}

/// `s` less a trailing parenthesized group whose trimmed contents pass
/// `inner`.
fn strip_paren_tail(s: &str, inner: impl Fn(&str) -> bool) -> Option<&str> {
    let body = s.trim_end().strip_suffix(')')?;
    let open = body.rfind('(')?;
    inner(body[open + 1..].trim()).then(|| body[..open].trim_end())
}

/// `N episode` or `N episodes`, any case.
fn is_episode_count(inner: &str) -> bool {
    let Some((count, word)) = inner.split_once(char::is_whitespace) else {
        return false;
    };
    let word = word.trim().to_ascii_lowercase();
    all_digits(count) && (word == "episode" || word == "episodes")
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
#[path = "history_forget_titles_test.rs"]
mod tests;
