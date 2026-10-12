//! The one way a command reads a show's external ids — the MAL and
//! AniList ids Kitsu's mappings carry for a Kitsu anime.
//!
//! Tracker id-resolution, the airing schedule, the banner and
//! episode-thumbnail backfills and aniskip all start from the same
//! `/anime/:id?include=mappings` answer, and a detail page with two
//! trackers connected asks for it twice at once. The answer is kept
//! in `meta_cache` under `kitsu:external-ids:v1:k<kitsu_id>`: thirty
//! days when both ids are present, a day when one is missing (see
//! [`external_ids_ttl`]). A failed read is kept nowhere — it says
//! nothing about the mapping.

use std::time::Duration;

use crate::app::AppState;
use crate::cache::ttl::{EXTERNAL_IDS_PARTIAL_TTL, EXTERNAL_IDS_TTL};
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::error::Result;
use crate::meta::kitsu::ExternalIds;

/// Cache key for one show's external ids. Versioned with the shape of
/// [`ExternalIds`]: a field added there moves the version so old rows
/// are not read as the new shape.
fn external_ids_key(kitsu_id: &str) -> String {
    format!("kitsu:external-ids:v1:k{kitsu_id}")
}

/// How long an answer from Kitsu's mappings is trusted. What is
/// present does not move, so a complete mapping — both trackers' ids
/// — is kept for [`EXTERNAL_IDS_TTL`]. What is missing may appear:
/// Kitsu adds the MAL mapping for a fresh seasonal show within days,
/// so an answer missing either id is re-asked after
/// [`EXTERNAL_IDS_PARTIAL_TTL`].
#[must_use]
pub(crate) fn external_ids_ttl(ids: &ExternalIds) -> Duration {
    if ids.mal.is_some() && ids.anilist.is_some() {
        EXTERNAL_IDS_TTL
    } else {
        EXTERNAL_IDS_PARTIAL_TTL
    }
}

/// The MAL and AniList ids Kitsu maps `kitsu_id` to, from the cache
/// when a row is live and from Kitsu otherwise.
///
/// # Errors
/// Whatever [`crate::meta::kitsu::KitsuClient::external_ids_for_kitsu_id`]
/// answers with: Network / Upstream / ParseFailed. A cache read that
/// fails is not an answer about the mapping and falls through to
/// Kitsu.
pub(crate) async fn external_ids_for(state: &AppState, kitsu_id: &str) -> Result<ExternalIds> {
    let key = external_ids_key(kitsu_id);
    if let Ok(Some(body)) = meta_cache_get(&state.cache_pool, &key) {
        if let Ok(ids) = serde_json::from_str::<ExternalIds>(&body) {
            return Ok(ids);
        }
        // A row this build cannot read is re-fetched and overwritten.
    }
    let ids = state.kitsu.external_ids_for_kitsu_id(kitsu_id).await?;
    if let Ok(body) = serde_json::to_string(&ids) {
        // Best-effort: a failed write costs one more Kitsu request.
        let _ = meta_cache_put(
            &state.cache_pool,
            &key,
            &body,
            external_ids_ttl(&ids).as_secs(),
        );
    }
    Ok(ids)
}

#[cfg(test)]
#[path = "external_ids_test.rs"]
mod tests;

#[cfg(test)]
#[path = "external_ids_prop_test.rs"]
mod prop_tests;
