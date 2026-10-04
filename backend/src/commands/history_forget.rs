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
//!
//! An expired cache row stays on disk until something overwrites it,
//! so expiry is not removal: deleting a row deletes all of these for
//! its show, and clearing the history deletes every one of them.

use crate::app::AppState;
use crate::cache::{meta_cache_delete, meta_cache_delete_prefix, meta_cache_entries_prefix};
use crate::commands::kitsu::{
    title_match_prefix, watched_at_key, ALLMANGA_KITSU_VERSION, TITLE_MATCH_VERSION,
};
use crate::error::{AniError, Result};
use crate::scraper::provider::ShowKey;

/// The prefixes every history-derived cache entry lives under.
const HISTORY_PREFIXES: [&str; 4] = ["watched-at:", "allmanga2kitsu:", "title-match:", "play:"];

/// Delete what the row for `id`, titled `title`, left behind.
///
/// # Errors
/// Cache or offsets-file write failures propagate.
pub(crate) fn forget_show(state: &AppState, id: &str, title: &str) -> Result<()> {
    let pool = &state.cache_pool;
    meta_cache_delete(pool, &watched_at_key(id))?;
    for version in 1..=ALLMANGA_KITSU_VERSION {
        meta_cache_delete(pool, &format!("allmanga2kitsu:v{version}:{id}"))?;
    }
    for prefix in title_match_prefixes(id, title) {
        forget_cours(state, &prefix)?;
    }
    forget_resolutions(state, id)?;
    crate::commands::anidb_offset::forget(state, id).map_err(|_| AniError::Io)
}

/// Delete everything every history row left behind.
///
/// # Errors
/// Cache or offsets-file write failures propagate.
pub(crate) fn forget_all(state: &AppState) -> Result<()> {
    for prefix in HISTORY_PREFIXES {
        meta_cache_delete_prefix(&state.cache_pool, prefix)?;
    }
    crate::commands::anidb_offset::forget_all(state).map_err(|_| AniError::Io)
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

/// Delete the keys under `prefix` whose remainder is a cour number.
/// The prefix ends where the cour starts, so a title that starts
/// another's ("Re", "Re:Creators") shares it.
fn forget_cours(state: &AppState, prefix: &str) -> Result<()> {
    for (key, _) in meta_cache_entries_prefix(&state.cache_pool, prefix)? {
        if all_digits(&key[prefix.len()..]) {
            meta_cache_delete(&state.cache_pool, &key)?;
        }
    }
    Ok(())
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

/// `title` less a trailing `(N episodes)`, itself optionally followed
/// by `(year)` — the tail rows written before the provider migration
/// carry, which Continue Watching strips before it searches.
fn without_episode_tail(title: &str) -> &str {
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
#[path = "history_forget_test.rs"]
mod tests;
