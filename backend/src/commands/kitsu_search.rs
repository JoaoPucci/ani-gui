//! Kitsu text search, cached through the SQLite `meta_cache` table.
//!
//! Each page is cached under its own offset (`kitsu:v3:search:<offset>:
//! <normalized-query>`); the plain search is the first page. Pages live
//! under `DISCOVERY_TTL` (6h): popular anime is stable, but trending
//! changes within a day, and that is the right tradeoff between
//! responsiveness and Kitsu API volume.
//!
//! Each page keeps its own TTL, so two pages of one query can come from
//! Kitsu rankings hours apart: a show that moved between them in the
//! meantime can appear on both — the renderer drops the second — or on
//! neither.

use crate::app::AppState;
use crate::cache::ttl::DISCOVERY_TTL;
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::commands::kitsu_warm::warm_signed_image_urls;
use crate::error::{AniError, Result};
use crate::meta::kitsu::KitsuAnimeRef;
use crate::meta::kitsu_page::KitsuSearchPage;

/// The search page size we ask Kitsu for. Kitsu's anime search caps
/// `page[limit]` at 20.
pub(crate) const SEARCH_PAGE_LIMIT: u8 = 20;

/// Search Kitsu for anime by free-text: the first page of
/// [`kitsu_search_page`], sharing its cache entry.
///
/// # Errors
/// Inherits from [`kitsu_search_page`].
pub async fn kitsu_search(state: &AppState, query: &str) -> Result<Vec<KitsuAnimeRef>> {
    Ok(kitsu_search_page(state, query, 0).await?.hits)
}

/// The deepest `page[offset]` a search walks to: the fifty-first page,
/// offsets 0 through 1000 in steps of [`SEARCH_PAGE_LIMIT`]. A text
/// search ranked by popularity has long stopped being about the query
/// by then, and the bound keeps a caller from walking Kitsu's whole
/// catalogue one page at a time.
pub(crate) const MAX_SEARCH_OFFSET: u32 = 1000;

/// One page of a Kitsu text search, starting at `offset`.
///
/// # Errors
/// - [`crate::error::AniError::InvalidSearchOffset`] when `offset` is
///   not a page boundary within [`MAX_SEARCH_OFFSET`].
/// - Inherits from [`crate::meta::kitsu::KitsuClient::search_page`] on
///   cache miss.
pub async fn kitsu_search_page(
    state: &AppState,
    query: &str,
    offset: u32,
) -> Result<KitsuSearchPage> {
    if offset % u32::from(SEARCH_PAGE_LIMIT) != 0 || offset > MAX_SEARCH_OFFSET {
        return Err(AniError::InvalidSearchOffset);
    }
    let key = format!("kitsu:v3:search:{offset}:{}", normalize_query(query));
    if let Some(body) = meta_cache_get(&state.cache_pool, &key)? {
        if let Ok(page) = serde_json::from_str::<KitsuSearchPage>(&body) {
            // Warm-on-hit: idempotent. get_or_fetch returns from
            // disk on bytes-cache hit (no network), so this is cheap.
            // Catches the case where meta_cache outlives the image
            // cache (LRU evicted the bytes, response still warm).
            warm_signed_image_urls(state, &body);
            return Ok(page);
        }
        // Cached body deserialized as something else — treat as miss and
        // rebuild rather than hand the frontend bad data.
    }
    let mut page = state
        .kitsu
        .search_page(query, SEARCH_PAGE_LIMIT, offset)
        .await?;
    // Never advertise a page this function would refuse.
    page.next_offset = page.next_offset.filter(|n| *n <= MAX_SEARCH_OFFSET);
    if let Ok(body) = serde_json::to_string(&page) {
        let _ = meta_cache_put(&state.cache_pool, &key, &body, DISCOVERY_TTL.as_secs());
        warm_signed_image_urls(state, &body);
    }
    Ok(page)
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
