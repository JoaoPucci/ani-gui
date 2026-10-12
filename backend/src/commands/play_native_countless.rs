//! The pick without a count signal — split from
//! `play_native_choice` for the per-file complexity bar: with no
//! episode count to score candidates by, the entry's titles and year
//! are all that vouch for a hit.

use crate::error::Result;
use crate::scraper::provider::{BrowseHit, Provider};

use super::play_native::PickedShow;
use super::play_native_title_grammar::normalized;
use super::play_native_title_marker::EntryTitles;

/// The pick without a count signal: an exact title beats positional
/// order, then a candidate whose part agrees with the entry's
/// ([`EntryTitles::part_agrees`]) beats one naming another part, and
/// within each a candidate whose own year matched Kitsu's beats the
/// rest — the order winner selection keeps. When the year disproved part of the pool and no survivor
/// carries positive identity evidence, the pool is token-search
/// garbage — reject it so the next alias gets its chance (the live
/// Tai-Ari mispick: three decades-off hits excluded, an unknown-year
/// movie left standing). A pool the year disproved nothing about
/// keeps the positional fallback — candidates without a matched year,
/// in the provider's order within each tier — so pages without season
/// links stay resolvable. An entry with no year either has nothing to
/// vouch for a hit but its name: the fallback then takes only a hit the
/// entry names ([`EntryTitles::names`]) — named, read and subtitled as
/// one of the entry's titles — so an announced sequel the provider does not list
/// yet is not handed the series its searches answer with. The single
/// probe still runs so the caller gets the episode list it needs.
pub(super) async fn pick_without_count<P: Provider + ?Sized>(
    client: &P,
    head: &[(&BrowseHit, bool)],
    needle: &str,
    year: YearEvidence,
    entry: EntryTitles<'_>,
) -> Result<PickedShow> {
    let exact = head
        .iter()
        .map(|(h, _)| *h)
        .filter(|h| normalized(&h.title) == needle);
    // Below an exact title, a candidate whose part agrees with the
    // entry's comes before one naming another part, as in winner
    // selection; within each, a matched year before the provider's
    // order. When the year disproved part of the pool, only a matched
    // year vouches for a candidate: the rest is token-search garbage
    // with no positional fallback.
    let tier = move |agrees: bool, confirmed: bool| {
        head.iter()
            .filter(move |(h, c)| *c == confirmed && entry.part_agrees(&h.title) == agrees)
            .filter(move |(h, _)| year != YearEvidence::Unknown || entry.names(&h.title))
            .map(|(h, _)| *h)
    };
    let positional_ok = year != YearEvidence::DisprovedSome;
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

/// What the entry's year said about a countless pool: nothing, for an
/// entry with no year; that some hit premiered too far from it; or
/// that it disproved none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum YearEvidence {
    /// The entry has no year: no hit can be vouched for by one.
    Unknown,
    /// The year disproved part of the pool.
    DisprovedSome,
    /// The year disproved nothing.
    DisprovedNone,
}

impl YearEvidence {
    /// The evidence for an entry's `year`, given whether the year
    /// filter excluded any hit.
    pub(super) fn of(year: Option<u32>, excluded_any: bool) -> Self {
        match (year, excluded_any) {
            (None, _) => Self::Unknown,
            (Some(_), true) => Self::DisprovedSome,
            (Some(_), false) => Self::DisprovedNone,
        }
    }
}

#[cfg(test)]
#[path = "play_native_countless_test.rs"]
mod tests;
