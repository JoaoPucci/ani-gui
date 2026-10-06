//! Choosing among a pick's survivors — split from the probe loop
//! for the per-file complexity bar. Two entry points: the countless
//! pick (no episode-count signal) and the winner selection over
//! probed candidates, plus the identity rank both the winner guard
//! and the probe loop's transport-death tracking share.

use crate::error::Result;
use crate::scraper::provider::{BrowseHit, EpisodeRef, Provider};

use super::play_native::PickedShow;
use super::play_native_title_marker::EntryTitles;

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

/// The rank a candidate is weighed by against the entry, in the
/// order winner selection prefers: 0 for an exact title; then, for a
/// candidate whose part agrees with the entry's
/// ([`EntryTitles::part_agrees`]), 1 with a matched year and 2
/// without; then 3 and 4 the same way for one naming another part.
/// An exact title or a matched year is identity — ranks 0 and odd.
pub(super) fn entry_rank(entry: EntryTitles<'_>, title: &str, needle: &str, confirmed: bool) -> u8 {
    let rank = identity_rank(title.trim().to_lowercase() == needle, confirmed);
    if rank == 0 || entry.part_agrees(title) {
        rank
    } else {
        rank + 2
    }
}

/// The strongest transport-dead candidate so far, by [`entry_rank`]
/// and then provider position, with the candidate that just died at
/// `pos` weighed in. A candidate the entry's titles refuse could never
/// have won, so its death blocks no winner; it still leaves the pool
/// unheard, which the caller records apart.
pub(super) fn strongest_dead(
    best: Option<(u8, usize)>,
    entry: EntryTitles<'_>,
    title: &str,
    needle: &str,
    confirmed: bool,
    pos: usize,
) -> Option<(u8, usize)> {
    if !entry.admits(title) {
        return best;
    }
    let failed = (entry_rank(entry, title, needle, confirmed), pos);
    Some(best.map_or(failed, |best| best.min(failed)))
}

/// The pick without a count signal: an exact title beats positional
/// order, then a candidate whose part agrees with the entry's
/// ([`EntryTitles::part_agrees`]) beats one naming another part, and
/// within each a candidate whose own year matched Kitsu's beats the
/// rest — the order winner selection keeps. When the year disproved part of the pool and no survivor
/// carries positive identity evidence, the pool is token-search
/// garbage — reject it so the next alias gets its chance (the live
/// Tai-Ari mispick: three decades-off hits excluded, an unknown-year
/// movie left standing). A pool the year disproved nothing about
/// keeps the provider's own ranking, so pages without season links
/// stay resolvable. The single probe still runs so the caller gets
/// the episode list it needs.
pub(super) async fn pick_without_count<P: Provider + ?Sized>(
    client: &P,
    head: &[(&BrowseHit, bool)],
    needle: &str,
    year_excluded_any: bool,
    entry: EntryTitles<'_>,
) -> Result<PickedShow> {
    let exact = head
        .iter()
        .map(|(h, _)| *h)
        .filter(|h| h.title.trim().to_lowercase() == needle);
    // Below an exact title, a candidate whose part agrees with the
    // entry's comes before one naming another part, as in winner
    // selection; within each, a matched year before the provider's
    // order. When the year disproved part of the pool, only a matched
    // year vouches for a candidate: the rest is token-search garbage
    // with no positional fallback.
    let tier = move |agrees: bool, confirmed: bool| {
        head.iter()
            .filter(move |(h, c)| *c == confirmed && entry.part_agrees(&h.title) == agrees)
            .map(|(h, _)| *h)
    };
    let positional_ok = !year_excluded_any;
    let positional = tier(true, true)
        .chain(tier(true, false).filter(move |_| positional_ok))
        .chain(tier(false, true))
        .chain(tier(false, false).filter(move |_| positional_ok));
    // Preference order, deduplicated by walking: a candidate whose
    // listing answers 404 is a stale slug, not the pool's verdict —
    // the next eligible candidate may carry the live listing.
    let mut seen: Vec<&str> = Vec::new();
    for chosen in exact.chain(positional) {
        if seen.contains(&chosen.slug.as_str()) {
            continue;
        }
        seen.push(&chosen.slug);
        match client.episodes(&chosen.slug).await {
            Ok(episodes) => {
                return Ok(PickedShow {
                    hit: chosen.clone(),
                    episodes,
                });
            }
            Err(e) if e.is_provider_block() || matches!(e, crate::error::AniError::GateRefused) => {
                return Err(e);
            }
            Err(crate::error::AniError::Upstream { .. }) => {}
            Err(e) => return Err(e),
        }
    }
    Err(crate::error::AniError::NoResults)
}

/// The winner among best-distance candidates, plus its rank
/// ([`entry_rank`]). An exact title match is the user's own words and stays
/// dominant; below it, a candidate whose part marker agrees with the
/// entry's ([`EntryTitles::part_agrees`]) outranks one that names
/// another part; below that, a detail year that matched Kitsu's
/// exactly outranks a merely tolerated neighbor; only a full tie
/// falls to provider order (min_by_key keeps the first of equals).
pub(super) fn select_winner(
    probed_ok: &[(&BrowseHit, Vec<EpisodeRef>, u32, bool)],
    best_dist: u32,
    needle: &str,
    entry: EntryTitles<'_>,
) -> (usize, u8) {
    let winner_idx = probed_ok
        .iter()
        .enumerate()
        .filter(|(_, (_, _, d, _))| *d == best_dist)
        .min_by_key(|(_, (h, _, _, confirmed))| {
            (
                h.title.trim().to_lowercase() != needle,
                !entry.part_agrees(&h.title),
                !*confirmed,
            )
        })
        .map(|(i, _)| i)
        .expect("best_dist came from this list");
    let (h, _, _, confirmed) = &probed_ok[winner_idx];
    (winner_idx, entry_rank(entry, &h.title, needle, *confirmed))
}
