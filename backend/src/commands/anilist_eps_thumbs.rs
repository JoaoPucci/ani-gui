//! Per-episode thumbnail backfill from AniList's `streamingEpisodes`
//! (Crunchyroll listings), used to fill nulls in Kitsu's episode
//! list. Keyed by `kitsu_id` so a cache hit skips BOTH the Kitsu
//! mappings round-trip AND the AniList GraphQL call — the
//! difference between "instant" and "a few seconds" on the home
//! Continue Watching strip after a cold start.
//!
//! The merge rule is "Kitsu wins when present" because Kitsu's
//! episode stills are consistently higher-quality than the
//! Crunchyroll promotional crops AniList carries.

use crate::app::AppState;
use crate::cache::ttl::{ANILIST_STREAMING_EPS_ERROR_TTL, ANILIST_STREAMING_EPS_TTL};
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::meta::anilist_media::MediaRef;
use crate::meta::anilist_streaming_eps::{streaming_listing_for_ids, StreamingListing};
use crate::meta::kitsu::{KitsuEpisode, KitsuEpisodeThumbnail};

#[path = "anilist_eps_fit.rs"]
mod fit;
#[path = "anilist_eps_titles.rs"]
mod titles;

/// Stable key for the per-show AniList episode-thumbnail backfill.
/// Versioned so a schema change orphans old rows: `v2` caches the
/// whole [`StreamingListing`] (entry titles, AniList's episode count
/// and status) where `v1` cached a bare number → thumbnail map.
fn anilist_eps_thumbs_key(kitsu_id: &str) -> String {
    format!("anilist:eps-thumbs:v2:k{kitsu_id}")
}

/// Writes the outcome of an AniList lookup to the cache. `Ok(listing)`
/// uses the full [`ANILIST_STREAMING_EPS_TTL`]; `Err(())` uses the
/// short [`ANILIST_STREAMING_EPS_ERROR_TTL`] so the next few minutes
/// of navigation hit cache instead of re-burning the rate-limit
/// budget on the same failed show.
fn cache_anilist_eps_thumbs(
    pool: &crate::cache::SqlitePool,
    kitsu_id: &str,
    outcome: &std::result::Result<StreamingListing, ()>,
) {
    let key = anilist_eps_thumbs_key(kitsu_id);
    let (listing, ttl) = match outcome {
        Ok(l) => (l.clone(), ANILIST_STREAMING_EPS_TTL.as_secs()),
        Err(()) => (
            StreamingListing::default(),
            ANILIST_STREAMING_EPS_ERROR_TTL.as_secs(),
        ),
    };
    if let Ok(body) = serde_json::to_string(&listing) {
        let _ = meta_cache_put(pool, &key, &body, ttl);
    }
}

/// Read-through cache for AniList streamingEpisodes thumbnails keyed
/// by `kitsu_id`. On hit, returns instantly with no network calls.
/// On miss, resolves Kitsu's mappings then fetches AniList, caches the
/// outcome (positive or negative), and returns it.
pub async fn thumbs_for_show(state: &AppState, kitsu_id: &str) -> StreamingListing {
    thumbs_for_show_with_anilist_base(state, kitsu_id, None).await
}

/// [`thumbs_for_show`] with the AniList endpoint overridable, so
/// tests can point the lookup at wiremock. Production passes `None`.
pub(crate) async fn thumbs_for_show_with_anilist_base(
    state: &AppState,
    kitsu_id: &str,
    anilist_base: Option<&str>,
) -> StreamingListing {
    let key = anilist_eps_thumbs_key(kitsu_id);
    if let Ok(Some(body)) = meta_cache_get(&state.cache_pool, &key) {
        if let Ok(listing) = serde_json::from_str::<StreamingListing>(&body) {
            return listing;
        }
    }
    let outcome = fetch_anilist_eps_thumbs(state, kitsu_id, anilist_base).await;
    cache_anilist_eps_thumbs(&state.cache_pool, kitsu_id, &outcome);
    outcome.unwrap_or_default()
}

/// One-shot lookup: `kitsu_id` → Kitsu's external-id mappings →
/// AniList streamingEpisodes → [`StreamingListing`]. Any failure step
/// (no usable mapping, AniList rate limit, parse failure) yields
/// `Err(())` so the caller can negative-cache an empty result.
async fn fetch_anilist_eps_thumbs(
    state: &AppState,
    kitsu_id: &str,
    anilist_base: Option<&str>,
) -> std::result::Result<StreamingListing, ()> {
    let ids = super::external_ids::external_ids_for(state, kitsu_id)
        .await
        .map_err(
            |e| tracing::warn!(kitsu_id, error = ?e, "anilist thumbs: mappings lookup failed"),
        )?;
    // Neither id: nothing to ask AniList. Err so the empty result
    // takes the short negative TTL, as a failed lookup does.
    MediaRef::preferring_mal(ids.mal, ids.anilist).ok_or(())?;
    streaming_listing_for_ids(&state.meta_http, ids.mal, ids.anilist, anilist_base)
        .await
        .map_err(|e| {
            tracing::warn!(
                kitsu_id,
                mal_id = ?ids.mal,
                anilist_id = ?ids.anilist,
                error = ?e,
                "anilist thumbs: streamingEpisodes fetch failed; negative-caching empty result",
            );
        })
}

/// Returns `true` when at least one episode in the page has a
/// numeric `number` and a missing `thumbnail.original` — i.e. the
/// only shape AniList's backfill can actually fill. Lets the caller
/// short-circuit the whole `thumbs_for_show` round-trip on pages
/// where Kitsu already covers every row, sparing the cold-cache
/// `/mappings` + AniList rate-limit cost when no merge would land.
pub fn needs_backfill(eps: &[KitsuEpisode]) -> bool {
    eps.iter().any(|ep| {
        ep.number.is_some()
            && ep
                .thumbnail
                .as_ref()
                .and_then(|t| t.original.as_ref())
                .is_none()
    })
}

/// Backfill `thumbnail.original` on Kitsu episodes from the AniList
/// streaming listing. Kitsu always wins when present. Episodes
/// without a `number` can't be matched and pass through unchanged,
/// and a listing that does not describe the show ([`fit::listing_fits`])
/// fills nothing — the page keeps its placeholder stills.
pub fn merge_thumbs(eps: Vec<KitsuEpisode>, anilist: &StreamingListing) -> Vec<KitsuEpisode> {
    if !fit::listing_fits(anilist, &eps) {
        return eps;
    }
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
            if let Some(entry) = anilist.entries.get(&num) {
                ep.thumbnail = Some(KitsuEpisodeThumbnail {
                    original: Some(entry.thumbnail.clone()),
                });
            }
            ep
        })
        .collect()
}

#[cfg(test)]
#[path = "anilist_eps_thumbs_test.rs"]
mod tests;
