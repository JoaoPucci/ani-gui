//! The pick without a count signal — split from
//! `play_native_choice` for the per-file complexity bar: with no
//! episode count to score candidates by, the title searched, a hit's
//! own year and the entry's name for it are all that vouch for a hit.

use crate::error::Result;
use crate::scraper::provider::{BrowseHit, Provider};

use super::play_native::PickedShow;
use super::play_native_title_grammar::normalized;
use super::play_native_title_marker::EntryTitles;

/// The pick without a count signal: an exact title beats everything;
/// below it a candidate whose part agrees with the entry's
/// ([`EntryTitles::part_agrees`]) beats one naming another part, and
/// within each a candidate whose own year matched Kitsu's beats the
/// rest, the provider's order breaking ties — the order winner
/// selection keeps. Only a hit something vouches for is taken at all:
/// the title searched, a matched year, or the entry's name for it
/// ([`EntryTitles::names`]) — named, read and subtitled as one of the
/// entry's titles, under any alias. A hit the year merely tolerated —
/// a year off, the December allowance, or no year on its page — is
/// taken on its name or not at all, so an announced sequel the
/// provider does not list yet is not handed the series its searches
/// answer with, whether the entry has a year or none (the live Ginga
/// Patrol mispick, with neither) and whether the year disproved some
/// other hit or none (the live Tai-Ari mispick: three decades-off hits
/// excluded, an unknown-year movie left standing, named by nothing). A
/// pool with no such hit is rejected so the next alias gets its
/// chance. The single probe still runs so the caller gets the episode
/// list it needs.
pub(super) async fn pick_without_count<P: Provider + ?Sized>(
    client: &P,
    head: &[(&BrowseHit, bool)],
    needle: &str,
    entry: EntryTitles<'_>,
) -> Result<PickedShow> {
    let exact = head
        .iter()
        .map(|(h, _)| *h)
        .filter(|h| normalized(&h.title) == needle);
    // Below an exact title, a candidate whose part agrees with the
    // entry's comes before one naming another part, as in winner
    // selection; within each, a matched year before the rest. A
    // candidate neither its year nor the entry's name vouches for is
    // never taken: the provider's order alone is no identity.
    let tier = move |agrees: bool, confirmed: bool| {
        head.iter()
            .filter(move |(h, c)| *c == confirmed && entry.part_agrees(&h.title) == agrees)
            .filter(move |(h, c)| *c || entry.names(&h.title))
            .map(|(h, _)| *h)
    };
    let positional = tier(true, true)
        .chain(tier(true, false))
        .chain(tier(false, true))
        .chain(tier(false, false));
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
                return Ok(PickedShow::listed(chosen.clone(), episodes));
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

#[cfg(test)]
#[path = "play_native_countless_test.rs"]
mod tests;
