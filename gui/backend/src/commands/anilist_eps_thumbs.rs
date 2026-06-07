//! Per-episode thumbnail backfill from AniList's `streamingEpisodes`
//! (Crunchyroll listings), used to fill nulls in Kitsu's episode
//! list. Keyed by `kitsu_id` so a cache hit skips BOTH the Kitsu
//! `/mappings` round-trip AND the AniList GraphQL call — the
//! difference between "instant" and "a few seconds" on the home
//! Continue Watching strip after a cold start.
//!
//! The merge rule is "Kitsu wins when present" because Kitsu's
//! episode stills are consistently higher-quality than the
//! Crunchyroll promotional crops AniList carries.

use std::collections::HashMap;

use crate::app::AppState;
use crate::cache::ttl::{ANILIST_STREAMING_EPS_ERROR_TTL, ANILIST_STREAMING_EPS_TTL};
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::meta::kitsu::{KitsuEpisode, KitsuEpisodeThumbnail};

/// Stable key for the per-show AniList episode-thumbnail backfill.
/// Versioned so a future schema change (e.g. richer cached payload)
/// can orphan old rows.
fn anilist_eps_thumbs_key(kitsu_id: &str) -> String {
    format!("anilist:eps-thumbs:v1:k{kitsu_id}")
}

/// Writes the outcome of an AniList lookup to the cache. `Ok(map)`
/// uses the full [`ANILIST_STREAMING_EPS_TTL`]; `Err(())` uses the
/// short [`ANILIST_STREAMING_EPS_ERROR_TTL`] so the next few minutes
/// of navigation hit cache instead of re-burning the rate-limit
/// budget on the same failed show.
fn cache_anilist_eps_thumbs(
    pool: &crate::cache::SqlitePool,
    kitsu_id: &str,
    outcome: &std::result::Result<HashMap<u32, String>, ()>,
) {
    let key = anilist_eps_thumbs_key(kitsu_id);
    let (map, ttl) = match outcome {
        Ok(m) => (m.clone(), ANILIST_STREAMING_EPS_TTL.as_secs()),
        Err(()) => (HashMap::new(), ANILIST_STREAMING_EPS_ERROR_TTL.as_secs()),
    };
    if let Ok(body) = serde_json::to_string(&map) {
        let _ = meta_cache_put(pool, &key, &body, ttl);
    }
}

/// Read-through cache for AniList streamingEpisodes thumbnails keyed
/// by `kitsu_id`. On hit, returns instantly with no network calls.
/// On miss, resolves the MAL id then fetches AniList, caches the
/// outcome (positive or negative), and returns it.
pub async fn thumbs_for_show(state: &AppState, kitsu_id: &str) -> HashMap<u32, String> {
    let key = anilist_eps_thumbs_key(kitsu_id);
    if let Ok(Some(body)) = meta_cache_get(&state.cache_pool, &key) {
        if let Ok(map) = serde_json::from_str::<HashMap<u32, String>>(&body) {
            return map;
        }
    }
    let outcome = fetch_anilist_eps_thumbs(state, kitsu_id).await;
    cache_anilist_eps_thumbs(&state.cache_pool, kitsu_id, &outcome);
    outcome.unwrap_or_default()
}

/// One-shot lookup: `kitsu_id` → `mal_id` → AniList streamingEpisodes
/// → `(ep_number, thumbnail_url)` map. Any failure step (no MAL
/// mapping, AniList rate limit, parse failure) yields `Err(())` so
/// the caller can negative-cache an empty result. The pair-list →
/// map dedup lives in `meta::anilist::streaming_eps_map_for_mal_id`
/// where its wiremock test suite covers the merge.
async fn fetch_anilist_eps_thumbs(
    state: &AppState,
    kitsu_id: &str,
) -> std::result::Result<HashMap<u32, String>, ()> {
    let mal_id = state
        .kitsu
        .mal_id_for_kitsu_id(kitsu_id)
        .await
        .map_err(|e| tracing::warn!(kitsu_id, error = ?e, "anilist thumbs: mal_id lookup failed"))?
        .ok_or(())?;
    crate::meta::anilist_streaming_eps::streaming_eps_map_for_mal_id(
        &state.proxy_http,
        mal_id,
        None,
    )
    .await
    .map_err(|e| {
        tracing::warn!(
            kitsu_id,
            mal_id,
            error = ?e,
            "anilist thumbs: streamingEpisodes fetch failed; negative-caching empty result",
        );
    })
}

/// Backfill `thumbnail.original` on Kitsu episodes from the AniList
/// `streamingEpisodes` map. Kitsu always wins when present. Episodes
/// without a `number` can't be matched and pass through unchanged.
pub fn merge_thumbs(eps: Vec<KitsuEpisode>, anilist: &HashMap<u32, String>) -> Vec<KitsuEpisode> {
    eps.into_iter()
        .map(|mut ep| {
            let already_has_thumb = ep
                .thumbnail
                .as_ref()
                .and_then(|t| t.original.as_ref())
                .is_some();
            if already_has_thumb {
                return ep;
            }
            let Some(num) = ep.number else { return ep };
            if let Some(url) = anilist.get(&num) {
                ep.thumbnail = Some(KitsuEpisodeThumbnail {
                    original: Some(url.clone()),
                });
            }
            ep
        })
        .collect()
}

#[cfg(test)]
#[path = "anilist_eps_thumbs_test.rs"]
mod tests;
