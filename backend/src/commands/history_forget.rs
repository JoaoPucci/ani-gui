//! What a history row leaves in the cache, removed with the row.
//!
//! A row records the Kitsu id of the show played. The cache holds the
//! same answer three more ways, keyed by the row's show id or title:
//! the show's watch stamp (`watched-at:`), its reverse mapping
//! (`allmanga2kitsu:`), and the title-match rows Continue Watching
//! stored for the row's title (`title-match:`). An expired cache row
//! stays on disk until something overwrites it, so expiry is not
//! removal: deleting a row deletes its entries, and clearing the
//! history deletes every entry under those prefixes, whichever version
//! of the app wrote them.

use crate::app::AppState;
use crate::cache::{meta_cache_delete, meta_cache_delete_prefix};
use crate::commands::kitsu::{allmanga_kitsu_key, title_match_prefix, watched_at_key};
use crate::error::Result;
use crate::scraper::provider::ShowKey;

/// The prefixes every history-derived cache entry lives under.
const HISTORY_PREFIXES: [&str; 3] = ["watched-at:", "allmanga2kitsu:", "title-match:"];

/// Delete what the row for `id`, titled `title`, left in the cache.
/// Title-match rows are keyed by the title Continue Watching searched,
/// which is the row's title less a legacy "(N episodes)" tail, under
/// every cour.
///
/// # Errors
/// Cache write failures propagate.
pub(crate) fn forget_show(state: &AppState, id: &str, title: &str) -> Result<()> {
    let pool = &state.cache_pool;
    meta_cache_delete(pool, &watched_at_key(id))?;
    meta_cache_delete(pool, &allmanga_kitsu_key(id))?;
    let provider = ShowKey::parse(id).provider;
    for searched in [title, without_episode_tail(title)] {
        meta_cache_delete_prefix(pool, &title_match_prefix(provider, searched))?;
    }
    Ok(())
}

/// Delete everything every history row left in the cache.
///
/// # Errors
/// Cache write failures propagate.
pub(crate) fn forget_all(state: &AppState) -> Result<()> {
    for prefix in HISTORY_PREFIXES {
        meta_cache_delete_prefix(&state.cache_pool, prefix)?;
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
