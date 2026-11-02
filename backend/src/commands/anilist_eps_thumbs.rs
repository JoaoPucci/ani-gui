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
use crate::cache::ttl::{ANILIST_STREAMING_EPS_NO_IDS_TTL, ANILIST_STREAMING_EPS_TTL};
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::meta::anilist_streaming_eps::StreamingListing;
use crate::meta::kitsu::{KitsuEpisode, KitsuEpisodeThumbnail};

#[path = "anilist_eps_contain.rs"]
mod contain;
#[path = "anilist_eps_fetch.rs"]
mod fetch;
#[path = "anilist_eps_fit.rs"]
mod fit;
use fetch::{fetch_anilist_eps_thumbs, Miss};
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
/// short [`ANILIST_STREAMING_EPS_NO_IDS_TTL`]: a show Kitsu maps to
/// neither tracker. A lookup that got no answer never comes here.
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
            ANILIST_STREAMING_EPS_NO_IDS_TTL.as_secs(),
        ),
    };
    if let Ok(body) = serde_json::to_string(&listing) {
        let _ = meta_cache_put(pool, &key, &body, ttl);
    }
}

/// Read-through cache for AniList streamingEpisodes thumbnails keyed
/// by `kitsu_id`. On hit, returns instantly with no network calls.
/// On miss, resolves Kitsu's mappings then fetches AniList, caches an
/// answer (a listing, or no ids to ask with), and returns it.
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
    // Two views filling the same show at once — the episode page and a
    // Continue Watching card — share one lookup: one read of Kitsu's
    // mappings, one AniList request. Keyed by the app state as well, so
    // two states (tests) never share.
    let shared = format!(
        "{:p}\u{0}{}\u{0}{kitsu_id}",
        std::ptr::from_ref(state),
        anilist_base.unwrap_or("")
    );
    THUMBS_IN_FLIGHT
        .run(shared, || {
            thumbs_for_show_once(state, kitsu_id, anilist_base)
        })
        .await
}

/// The thumbnail lookups in flight, by show.
static THUMBS_IN_FLIGHT: std::sync::LazyLock<crate::meta::inflight::Inflight<StreamingListing>> =
    std::sync::LazyLock::new(Default::default);

/// One read-through of the show's thumbnail listing.
async fn thumbs_for_show_once(
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
    match fetch_anilist_eps_thumbs(state, kitsu_id, anilist_base).await {
        // A step that got no answer says nothing about the show:
        // nothing is kept, and the next view asks again.
        Err(Miss::Unanswered) => StreamingListing::default(),
        outcome => {
            let outcome = outcome.map_err(|_| ());
            cache_anilist_eps_thumbs(&state.cache_pool, kitsu_id, &outcome);
            outcome.unwrap_or_default()
        }
    }
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
            if let Some(url) = anilist.entries.get(&num).and_then(|e| e.thumbnail.as_ref()) {
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
