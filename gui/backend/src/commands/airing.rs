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

/// Grace past the scheduled airing before the cache row must die.
/// AniList flips `nextAiringEpisode` within minutes of the drop; a
/// short overlap avoids refetch-hammering while that propagates.
const AIRING_GRACE_SECS: u64 = 10 * 60;

/// Ceiling for rows whose next airing is far away. The aired count
/// cannot move before the scheduled airing, so re-asking every 3h
/// buys nothing — a daily ceiling keeps AniList rate-limit pressure
/// ~8x lower while the schedule cap still expires the row right
/// after a drop.
const AIRING_TTL_MAX_SECS: u64 = 24 * 60 * 60;

/// Cache TTL for one airing row. A row written shortly before a
/// scheduled airing must not outlive the airing by the full fixed
/// window — the just-dropped episode would stay greyed out until the
/// TTL expired (Codex P2 #3565710322). Cap at the schedule boundary
/// plus a short grace; an already-passed timestamp (stale AniList
/// row, clock skew) collapses to the grace so the next read soon
/// refetches without hammering per-request.
fn airing_ttl_for(next_airing_at: Option<u64>, now_epoch_s: u64) -> u64 {
    match next_airing_at {
        Some(at) if at <= now_epoch_s => AIRING_GRACE_SECS,
        Some(at) => (at - now_epoch_s + AIRING_GRACE_SECS).min(AIRING_TTL_MAX_SECS),
        None => AIRING_TTL_SECS,
    }
}

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
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let ttl = airing_ttl_for(status.next_airing_at, now);
        meta_cache_put(&state.cache_pool, &key, &body, ttl)?;
    }
    Ok(status)
}

#[cfg(test)]
#[path = "airing_test.rs"]
mod tests;
