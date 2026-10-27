//! The listing row of every AniList media recording a MAL id, which the
//! episode map reads for ownership. Split from [`super`] so each file
//! stays inside the CRAP gate's per-file bar.

use crate::app::AppState;
use crate::cache::ttl::{ANILIST_ID_MAP_MISS_TTL, ANILIST_ID_MAP_TTL};
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::error::Result;

fn recording_key(mal_id: u32) -> String {
    format!("anilist:media-ids-recording-mal:v1:{mal_id}")
}

/// Every AniList media that records MAL's `mal_id`, in AniList id
/// order; empty when none does. Kept under its own row, its ids joined
/// by commas (empty for none), for as long as the single translations.
///
/// # Errors
/// Same as [`crate::meta::anilist::media_ids_recording_mal`]. Nothing is
/// remembered from a failed request.
pub(crate) async fn media_ids_recording_mal(
    state: &AppState,
    mal_id: u32,
    anilist_base: Option<&str>,
) -> Result<Vec<u32>> {
    let key = recording_key(mal_id);
    if let Ok(Some(body)) = meta_cache_get(&state.cache_pool, &key) {
        let ids: Option<Vec<u32>> = body
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().ok())
            .collect();
        if let Some(ids) = ids {
            return Ok(ids);
        }
    }
    let ids = crate::meta::anilist::media_ids_recording_mal(&state.meta_http, mal_id, anilist_base)
        .await?;
    let body: Vec<String> = ids.iter().map(u32::to_string).collect();
    let ttl = if ids.is_empty() {
        ANILIST_ID_MAP_MISS_TTL
    } else {
        ANILIST_ID_MAP_TTL
    };
    let _ = meta_cache_put(&state.cache_pool, &key, &body.join(","), ttl.as_secs());
    Ok(ids)
}

#[cfg(test)]
#[path = "anilist_ids_listing_test.rs"]
mod tests;
