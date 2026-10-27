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
    /// Kitsu answered that the show maps to neither AniList nor MAL:
    /// an answer, negative-cached.
    NoIds,
    /// A step got no answer — Kitsu's mappings or AniList failing,
    /// refusing this client, or answering what cannot be read. Unknown,
    /// and never cached as an absence.
    Unanswered,
}

/// One-shot lookup: `kitsu_id` → Kitsu's external-id mappings →
/// AniList streamingEpisodes → [`StreamingListing`]. A show with
/// neither id is [`Miss::NoIds`], which the caller negative-caches;
/// any step without an answer is [`Miss::Unanswered`], which it does
/// not.
pub(super) async fn fetch_anilist_eps_thumbs(
    state: &AppState,
    kitsu_id: &str,
    anilist_base: Option<&str>,
) -> std::result::Result<StreamingListing, Miss> {
    let ids = crate::commands::external_ids::external_ids_for(state, kitsu_id)
        .await
        .map_err(|e| {
            tracing::warn!(kitsu_id, error = ?e, "anilist thumbs: mappings lookup failed");
            Miss::Unanswered
        })?;
    // Neither id: nothing to ask AniList, and that is Kitsu's answer.
    MediaRef::preferring_mal(ids.mal, ids.anilist).ok_or(Miss::NoIds)?;
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
        tracing::warn!(
            kitsu_id,
            mal_id = ?ids.mal,
            anilist_id = ?ids.anilist,
            error = ?e,
            "anilist thumbs: streamingEpisodes fetch got no answer; nothing cached",
        );
        Miss::Unanswered
    })
}
