//! Airing-status command — bridges a Kitsu id to AniList's airing
//! schedule so the detail page can grey out unaired episode tiles.
//!
//! Resolution reuses the tracker mapping chain: Kitsu's mappings give
//! the `anilist/anime` id (present even on fresh seasonal shows Kitsu
//! hasn't MAL-mapped) or the MAL id, and [`crate::meta::anilist::
//! airing_status`] accepts either. No mapping at all → the default
//! all-`None` status, which the UI treats as "unknown, don't gate".

use crate::app::AppState;
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::error::Result;
use crate::meta::anilist::AiringStatus;

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
    // Green commit fills the bridge + cache in.
    let _ = (state, kitsu_id, anilist_base);
    Ok(AiringStatus::default())
}

#[cfg(test)]
#[path = "airing_test.rs"]
mod tests;
