//! Warming a Continue Watching strip's AniList fills at once.
//!
//! Each Continue card asks for its episodes, and its show's detail,
//! once its own row is ready, and on a cold cache those rows land
//! seconds apart: each card's thumbnail and banner lookup then misses
//! on its own, and the lanes that would batch them
//! ([`crate::meta::anilist_batch`]) mostly carry one or two shows. The
//! history records most rows' Kitsu id, so Home hands the whole strip
//! here as it loads. The warm reads every show's mappings together,
//! then starts every show's lookups in the same moment, gathered
//! (`meta::anilist_batch::gathered`), so each kind goes as one batch. A
//! card's own
//! lookup that comes while the warm's is out shares it (stills and
//! banners are both shared per show), or finds its answer cached; one
//! that arrives while a warm's lane is open rides its page, up to the
//! window later than it would have gone alone. A card whose own lookup
//! comes before the warm's — the warm waits for every show's mappings —
//! goes alone, and the warm's lookup for that show shares it.
//!
//! Every lookup counts as a background fill, as the cards' do, and the
//! warm caches only what they would: the thumbnail listing under its
//! row, and AniList's answer for a show's banner — the banner, or that
//! it has none — where the detail read looks for it when Kitsu has no
//! cover (`anilist_banner_memo::take_warmed`), once. A
//! warm whose caller has gone is dropped with it.

use crate::app::AppState;
use crate::cache::meta_cache_get;

/// The most shows a warm takes: one page of a batch.
pub(crate) const MAX_SHOWS: usize = crate::meta::anilist_pages::PAGE_CAP;

/// The lane every AniList fill of `state` against `anilist_base` joins.
pub(crate) fn fill_lane(state: &AppState, anilist_base: Option<&str>) -> String {
    format!(
        "{:p}\u{0}{}",
        std::ptr::from_ref(state),
        anilist_base.unwrap_or("")
    )
}

/// Warm the stills and banners of the strip's shows `kitsu_ids`: at
/// most [`MAX_SHOWS`], each once, ids that are not Kitsu ids left out.
pub(crate) async fn warm_strip(state: &AppState, kitsu_ids: &[String], anilist_base: Option<&str>) {
    let mut ids: Vec<&str> = Vec::new();
    for raw in kitsu_ids {
        if let Ok(id) = crate::kitsu_id::require(raw) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        if ids.len() == MAX_SHOWS {
            break;
        }
    }
    // Every show's mappings first, together: with them in hand each
    // lookup below reaches its lane in the same moment.
    futures_util::future::join_all(
        ids.iter()
            .map(|id| crate::commands::external_ids::external_ids_for(state, id)),
    )
    .await;
    // Gathered: the warm's lookups go as one page per kind, none sent
    // ahead alone. A card's own lookup still opens a lane at once.
    let stills = futures_util::future::join_all(ids.iter().map(|id| {
        crate::commands::anilist_eps_thumbs::thumbs_for_show_with_anilist_base(
            state,
            id,
            anilist_base,
        )
    }));
    let banners =
        futures_util::future::join_all(ids.iter().map(|id| warm_banner(state, id, anilist_base)));
    crate::meta::anilist_batch::gathered(futures_util::future::join(stills, banners)).await;
}

/// Remember the banner AniList has for `kitsu_id`, unless its detail is
/// already kept (its banner, or its cover, decided) or one is
/// remembered.
async fn warm_banner(state: &AppState, kitsu_id: &str, anilist_base: Option<&str>) {
    let detail = crate::commands::kitsu::anime_detail_key(kitsu_id);
    if matches!(meta_cache_get(&state.cache_pool, &detail), Ok(Some(_)))
        || crate::commands::anilist_banner_memo::remembered(&state.cache_pool, kitsu_id).is_some()
        || crate::commands::anilist_banner_memo::warmed(&state.cache_pool, kitsu_id).is_some()
    {
        return;
    }
    // Shared with a card's detail read asking for the same banner.
    let banner =
        crate::commands::anilist_banner_fill::banner_for_show(state, kitsu_id, anilist_base).await;
    // An answer is kept, banner or none; no answer, nothing.
    if let Ok(found) = banner {
        crate::commands::anilist_banner_memo::remember_warmed(
            &state.cache_pool,
            kitsu_id,
            found.as_deref(),
        );
    }
}

#[cfg(test)]
#[path = "strip_warm_test.rs"]
mod tests;
