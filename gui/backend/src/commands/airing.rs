//! Airing-status command — bridges a Kitsu id to AniList's airing
//! schedule so the detail page can grey out unaired episode tiles.
//!
//! Resolution reuses the tracker mapping chain: Kitsu's mappings give
//! the `anilist/anime` id (present even on fresh seasonal shows Kitsu
//! hasn't MAL-mapped) or the MAL id, and [`crate::meta::anilist_airing::
//! airing_status`] accepts either. No mapping at all → the default
//! all-`None` status, which the UI treats as "unknown, don't gate".

use crate::app::AppState;
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::error::Result;
use crate::meta::anilist_airing::AiringStatus;

/// 3 hours. Airing data only moves once per weekly episode, but the
/// aired count must tick up reasonably soon after a premiere — a
/// longer TTL would keep a just-aired episode greyed out for hours.
const AIRING_TTL_SECS: u64 = 3 * 60 * 60;

/// Fetch the airing status for a Kitsu anime id. Cached per show.
/// Unknown (unmapped show, AniList doesn't index it) collapses to the
/// default all-`None` [`AiringStatus`] — a non-error the UI renders
/// ungated.
///
/// # Errors
/// Network / Upstream / ParseFailed from the underlying clients.
pub async fn airing_get(state: &AppState, kitsu_id: &str) -> Result<AiringStatus> {
    airing_get_with_anilist_base(state, kitsu_id, None).await
}

/// [`airing_get`] with the AniList endpoint override exposed for
/// tests. Production passes `None` via the public wrapper.
pub(crate) async fn airing_get_with_anilist_base(
    state: &AppState,
    kitsu_id: &str,
    anilist_base: Option<&str>,
) -> Result<AiringStatus> {
    // v2: AiringStatus gained `upcoming`; serde(default) parses v1 rows
    // fine but they would render dateless tiles for up to the TTL.
    let key = format!("airing:v2:{kitsu_id}");
    if let Some(body) = meta_cache_get(&state.cache_pool, &key)? {
        if let Ok(status) = serde_json::from_str::<AiringStatus>(&body) {
            return Ok(status);
        }
        // Corrupt cache row — fall through to refetch.
    }

    let ids = state.kitsu.external_ids_for_kitsu_id(kitsu_id).await?;
    let status = if ids.anilist.is_none() && ids.mal.is_none() {
        AiringStatus::default()
    } else {
        crate::meta::anilist_airing::airing_status(
            &state.proxy_http,
            ids.anilist,
            ids.mal,
            anilist_base,
        )
        .await?
        .unwrap_or_default()
    };

    if let Ok(body) = serde_json::to_string(&status) {
        meta_cache_put(&state.cache_pool, &key, &body, AIRING_TTL_SECS)?;
    }
    Ok(status)
}

#[cfg(test)]
#[path = "airing_test.rs"]
mod tests;
