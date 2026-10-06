//! Fitting a probed pool to the entry it is picked for, by the
//! entry's own titles — split from `play_native` for the per-file
//! complexity bar.
//!
//! Two shapes the episode count alone gets wrong:
//!
//! - **A sibling named for another season or part.** When the
//!   provider's search leaves the entry's own listing out of a pool,
//!   its sequel can sit within the count tolerance and a year of the
//!   entry ("My Star: Season 2" for `[Oshi no Ko]`). A candidate whose
//!   title the entry's titles do not admit
//!   ([`EntryTitles::admits`]) is still probed — it is evidence about
//!   the pool — but it can never be picked.
//! - **One listing that spans this entry and the next.** Kitsu keeps
//!   "Attack on Titan Season 3" (12) and its "Part 2" (10) apart;
//!   hianime lists a 22-episode "Season 3" beside a 10-episode
//!   "Season 3 Part 2". Scored on count the Part 2 listing wins the
//!   first entry. The spanning listing is recognised by the sibling
//!   that completes it — one whose stem starts with the spanning
//!   listing's, whose count is exactly what that listing holds
//!   beyond this entry — and is cut to this entry's episodes, under
//!   the listing's own numbers.

use crate::scraper::provider::{BrowseHit, EpisodeRef};

use super::play_native_numbering::regular_episode_count;
use super::play_native_title_marker::{stem, EntryTitles};

/// The distance a candidate that may not be picked is scored at:
/// beyond every tolerance, so it never wins on count and never sets
/// the pool's best distance.
pub(super) const UNFIT: u32 = u32::MAX;

/// A probed candidate as the pick scores it: the hit, its listing,
/// its count distance from the entry, and whether its own year
/// matched the entry's.
pub(super) type Probed<'h> = (&'h BrowseHit, Vec<EpisodeRef>, u32, bool);

/// Fit the probed pool to the entry: a candidate the entry's titles
/// do not admit is scored [`UNFIT`], and a listing that spans this
/// entry and the next is cut to this entry's episodes and scored as
/// the exact fit it then is, the sibling completing it scored
/// [`UNFIT`] — that sibling is the next entry, not this one.
///
/// Candidates keep their listings, years and places, so a rule that
/// reads the pool as parts of one entry still sees every part.
///
/// Returns whether the titles refused a candidate the count alone
/// would have accepted: a pool rejected after that is
/// [`super::play_native_title_verdict::refused_by_title`], not a clean
/// miss.
pub(super) fn fit_to_entry(
    probed: &mut [Probed<'_>],
    expected: u32,
    entry: EntryTitles<'_>,
) -> bool {
    let admitted: Vec<bool> = probed
        .iter()
        .map(|(h, _, _, _)| entry.admits(&h.title))
        .collect();
    let tolerance = super::play_native::ep_count_threshold(expected);
    let refused_a_fit = probed
        .iter()
        .zip(&admitted)
        .any(|(row, ok)| !ok && row.2 <= tolerance);
    let span = spanning(probed, expected, &admitted);
    for (row, ok) in probed.iter_mut().zip(&admitted) {
        if !ok {
            row.2 = UNFIT;
        }
    }
    if let Some((wide, later)) = span {
        for j in later {
            probed[j].2 = UNFIT;
        }
        probed[wide].1 = head_of(&probed[wide].1, expected);
        probed[wide].2 = 0;
    }
    refused_a_fit
}

/// The listing that spans this entry and the next, with the later
/// parts that complete it. It must be admitted, carry the entry's own
/// year, and list more than the entry has; a sibling completing it
/// must have a stem that starts with its stem and list exactly the
/// remainder. A listing that already
/// fits exactly elsewhere in the pool is the entry's own, and nothing
/// is cut.
fn spanning(
    probed: &[Probed<'_>],
    expected: u32,
    admitted: &[bool],
) -> Option<(usize, Vec<usize>)> {
    let counts: Vec<u32> = probed
        .iter()
        .map(|(_, eps, _, _)| regular_episode_count(eps))
        .collect();
    let (wide, later) = (0..probed.len()).find_map(|m| {
        let (h, _, _, confirmed) = &probed[m];
        if !admitted[m] || !confirmed || counts[m] <= expected {
            return None;
        }
        let own = stem(&h.title);
        let later: Vec<usize> = (0..probed.len())
            .filter(|&j| {
                j != m
                    && counts[j] == counts[m] - expected
                    && extends(&stem(&probed[j].0.title), &own)
            })
            .collect();
        (!own.is_empty() && !later.is_empty()).then_some((m, later))
    })?;
    let dedicated = (0..probed.len())
        .any(|k| k != wide && !later.contains(&k) && admitted[k] && probed[k].2 == 0);
    (!dedicated).then_some((wide, later))
}

/// Whether `longer` starts with every word of `stem`.
fn extends(longer: &[String], stem: &[String]) -> bool {
    longer.len() >= stem.len() && longer[..stem.len()] == *stem
}

/// The listing's rows up to its `expected + 1`-th regular episode:
/// the entry's episodes with any recap tagged among or right after
/// them.
fn head_of(episodes: &[EpisodeRef], expected: u32) -> Vec<EpisodeRef> {
    let mut regular = 0;
    let mut out = Vec::new();
    for e in episodes {
        if regular_episode_count(std::slice::from_ref(e)) == 1 {
            if regular == expected {
                break;
            }
            regular += 1;
        }
        out.push(e.clone());
    }
    out
}

#[cfg(test)]
#[path = "play_native_wide_listing_test.rs"]
mod tests;
