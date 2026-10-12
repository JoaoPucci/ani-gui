//! Kitsu text search, cached through the SQLite `meta_cache` table.
//!
//! Search results live under `DISCOVERY_TTL` (6h): popular anime is
//! stable, but trending changes within a day, and that is the right
//! tradeoff between responsiveness and Kitsu API volume.

use crate::app::AppState;
use crate::cache::ttl::DISCOVERY_TTL;
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::commands::kitsu_warm::warm_signed_image_urls;
use crate::error::Result;
use crate::meta::kitsu::{drop_music, KitsuAnimeRef};

/// The search page size we ask Kitsu for. Kitsu's anime search caps
/// `page[limit]` at 20.
pub(crate) const SEARCH_PAGE_LIMIT: u8 = 20;

/// Search Kitsu for anime by free-text. Cache key:
/// `kitsu:v2:search:<normalized-query>`. Returns at most
/// [`SEARCH_PAGE_LIMIT`] hits.
///
/// # Errors
/// Inherits from [`crate::meta::kitsu::KitsuClient::search`] on cache miss.
pub async fn kitsu_search(state: &AppState, query: &str) -> Result<Vec<KitsuAnimeRef>> {
    let key = format!("kitsu:v2:search:{}", normalize_query(query));
    if let Some(body) = meta_cache_get(&state.cache_pool, &key)? {
        if let Ok(hits) = serde_json::from_str::<Vec<KitsuAnimeRef>>(&body) {
            // Warm-on-hit: idempotent. get_or_fetch returns from
            // disk on bytes-cache hit (no network), so this is cheap.
            // Catches the case where meta_cache outlives the image
            // cache (LRU evicted the bytes, response still warm).
            warm_signed_image_urls(state, &body);
            // Sanitize music on read so caches written before the music
            // filter existed don't keep serving MVs.
            return Ok(drop_music(hits));
        }
        // Cached body deserialized as something else — treat as miss and
        // rebuild rather than hand the frontend bad data.
    }
    let hits = state.kitsu.search(query, SEARCH_PAGE_LIMIT).await?;
    if let Ok(body) = serde_json::to_string(&hits) {
        let _ = meta_cache_put(&state.cache_pool, &key, &body, DISCOVERY_TTL.as_secs());
        warm_signed_image_urls(state, &body);
    }
    Ok(hits)
}

/// Lowercase + collapse internal whitespace + trim. Stable cache key
/// regardless of how the user types the query.
fn normalize_query(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
#[path = "kitsu_search_test.rs"]
mod tests;
