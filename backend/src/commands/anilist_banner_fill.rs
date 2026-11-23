//! One show's AniList banner lookup, shared: the detail read's backfill
//! and a Continue strip's warm ask for a show's banner through here, so
//! a detail read while the warm's lookup is out takes its answer
//! instead of asking AniList again. Each lookup goes through the
//! banner lane ([`crate::meta::anilist_media::banner_coalesced`]) as a
//! background fill. Nothing is remembered once the answer is out:
//! whatever keeps it is the caller's business.

use crate::app::AppState;
use crate::error::Result;

/// The banner lookups in flight, by app state, endpoint and show.
static BANNERS_IN_FLIGHT: std::sync::LazyLock<
    crate::meta::inflight::Inflight<Result<Option<String>>>,
> = std::sync::LazyLock::new(Default::default);

/// AniList's `bannerImage` for a Kitsu anime, reached through the ids
/// Kitsu's mappings carry. `Ok(None)` when AniList has no banner or no
/// media; `Err` when a step got no answer.
///
/// # Errors
/// Kitsu's mappings read failing, or AniList's lookup getting no answer.
pub(crate) async fn banner_for_show(
    state: &AppState,
    kitsu_id: &str,
    anilist_base: Option<&str>,
) -> Result<Option<String>> {
    let lane = crate::commands::strip_warm::fill_lane(state, anilist_base);
    let shared = shared_key(&lane, kitsu_id);
    BANNERS_IN_FLIGHT
        .run(shared, || async {
            let ids = crate::commands::external_ids::external_ids_for(state, kitsu_id).await?;
            // Neither id: nothing to ask AniList, as the single lookup
            // asks nothing; no lane is opened for it.
            if ids.mal.is_none() && ids.anilist.is_none() {
                return Ok(None);
            }
            let want = crate::meta::anilist_batch::Want {
                mal: ids.mal,
                anilist: ids.anilist,
            };
            // The detail renders without it: a background fill.
            crate::meta::anilist_gate::in_background(crate::meta::anilist_media::banner_coalesced(
                &state.meta_http,
                &lane,
                want,
                anilist_base,
            ))
            .await
        })
        .await
}

/// The key a show's banner lookup is shared under: per lane and show.
fn shared_key(lane: &str, kitsu_id: &str) -> String {
    format!("{lane}\u{0}{kitsu_id}")
}

/// How many callers wait on `kitsu_id`'s banner lookup now, beside the
/// one running it.
#[cfg(test)]
pub(crate) fn sharing(state: &AppState, kitsu_id: &str, anilist_base: Option<&str>) -> usize {
    let lane = crate::commands::strip_warm::fill_lane(state, anilist_base);
    BANNERS_IN_FLIGHT.sharing(&shared_key(&lane, kitsu_id))
}
