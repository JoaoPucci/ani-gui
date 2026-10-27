//! The miss path of the episode-thumbnail backfill: Kitsu's mappings,
//! then AniList's listing, and why a lookup came back without one.
//! Split from [`super`] so each file stays inside the CRAP gate's
//! per-file bar.

use crate::app::AppState;
use crate::meta::anilist_media::MediaRef;
use crate::meta::anilist_streaming_eps::{streaming_listing_for_ids, StreamingListing};

/// Why a lookup came back without a listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Miss {
    /// A step answered and the answer was no listing — no usable
    /// mapping, an unreadable body, an upstream error: negative-cached.
    Failed,
    /// AniList refused this client, or its budget held the request
    /// back. Not an answer, and never cached as one.
    RateLimited,
}

/// One-shot lookup: `kitsu_id` → Kitsu's external-id mappings →
/// AniList streamingEpisodes → [`StreamingListing`]. A failed step
/// (no usable mapping, parse failure) is [`Miss::Failed`], which the
/// caller negative-caches; an AniList rate limit is
/// [`Miss::RateLimited`], which it does not.
pub(super) async fn fetch_anilist_eps_thumbs(
    state: &AppState,
    kitsu_id: &str,
    anilist_base: Option<&str>,
) -> std::result::Result<StreamingListing, Miss> {
    let ids = crate::commands::external_ids::external_ids_for(state, kitsu_id)
        .await
        .map_err(|e| {
            tracing::warn!(kitsu_id, error = ?e, "anilist thumbs: mappings lookup failed");
            Miss::Failed
        })?;
    // Neither id: nothing to ask AniList. Err so the empty result
    // takes the short negative TTL, as a failed lookup does.
    MediaRef::preferring_mal(ids.mal, ids.anilist).ok_or(Miss::Failed)?;
    // A fill the page renders without: it spends AniList's budget
    // behind what the user is looking at.
    crate::meta::anilist_gate::in_background(streaming_listing_for_ids(
        &state.meta_http,
        ids.mal,
        ids.anilist,
        anilist_base,
    ))
    .await
    .map_err(|e| {
        let miss = if matches!(e, crate::error::AniError::RateLimited { .. }) {
            Miss::RateLimited
        } else {
            Miss::Failed
        };
        tracing::warn!(
            kitsu_id,
            mal_id = ?ids.mal,
            anilist_id = ?ids.anilist,
            error = ?e,
            ?miss,
            "anilist thumbs: streamingEpisodes fetch failed",
        );
        miss
    })
}
