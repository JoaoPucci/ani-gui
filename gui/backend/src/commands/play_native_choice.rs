//! Choosing among a pick's survivors — split from the probe loop
//! for the per-file complexity bar. Two entry points: the countless
//! pick (no episode-count signal) and the winner selection over
//! probed candidates, plus the identity rank both the winner guard
//! and the probe loop's transport-death tracking share.

use crate::error::Result;
use crate::scraper::anidb::{AnidbClient, AnidbFetch, BrowseHit};

use super::play_native::PickedShow;

/// Identity a candidate carries: 0 = exact title, 1 = year-confirmed,
/// 2 = neither. Lower outranks higher.
pub(super) fn identity_rank(title_matches: bool, confirmed: bool) -> u8 {
    if title_matches {
        0
    } else if confirmed {
        1
    } else {
        2
    }
}

/// The pick without a count signal: an exact title beats positional
/// order, then a candidate whose own year matched Kitsu's beats the
/// rest. When the year disproved part of the pool and no survivor
/// carries positive identity evidence, the pool is token-search
/// garbage — reject it so the next alias gets its chance (the live
/// Tai-Ari mispick: three decades-off hits excluded, an unknown-year
/// movie left standing). A pool the year disproved nothing about
/// keeps the provider's own ranking, so pages without season links
/// stay resolvable. The single probe still runs so the caller gets
/// the episode list it needs.
pub(super) async fn pick_without_count<F: AnidbFetch>(
    client: &AnidbClient<F>,
    head: &[(&BrowseHit, bool)],
    needle: &str,
    year_excluded_any: bool,
) -> Result<PickedShow> {
    let exact = head
        .iter()
        .map(|(h, _)| *h)
        .find(|h| h.title.trim().to_lowercase() == needle);
    let confirmed = head.iter().find(|(_, c)| *c).map(|(h, _)| *h);
    let chosen = match (exact, confirmed) {
        (Some(h), _) => h,
        (None, Some(h)) => h,
        (None, None) if year_excluded_any => {
            return Err(crate::error::AniError::NoResults);
        }
        (None, None) => head
            .first()
            .map(|(h, _)| *h)
            .ok_or(crate::error::AniError::NoResults)?,
    };
    let episodes = client.episodes(&chosen.slug).await?;
    Ok(PickedShow {
        hit: chosen.clone(),
        episodes,
    })
}

/// The winner among best-distance candidates, plus its identity
/// rank. An exact title match is the user's own words and stays
/// dominant; below it, a detail year that matched Kitsu's exactly
/// outranks a merely tolerated neighbor; only a full tie falls to
/// provider order (min_by_key keeps the first of equals).
pub(super) fn select_winner(
    probed_ok: &[(
        &BrowseHit,
        Vec<crate::scraper::anidb::EpisodeRef>,
        u32,
        bool,
    )],
    best_dist: u32,
    needle: &str,
) -> (usize, u8) {
    let winner_idx = probed_ok
        .iter()
        .enumerate()
        .filter(|(_, (_, _, d, _))| *d == best_dist)
        .min_by_key(|(_, (h, _, _, confirmed))| {
            (h.title.trim().to_lowercase() != needle, !*confirmed)
        })
        .map(|(i, _)| i)
        .expect("best_dist came from this list");
    let (h, _, _, confirmed) = &probed_ok[winner_idx];
    let rank = identity_rank(h.title.trim().to_lowercase() == needle, *confirmed);
    (winner_idx, rank)
}
