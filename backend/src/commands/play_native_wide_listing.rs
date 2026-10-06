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
//!   that completes it — one that names a later part right after the
//!   spanning listing's stem, and whose count is exactly what that
//!   listing holds beyond this entry — and is cut to this entry's
//!   episodes, under the listing's own numbers.

use crate::scraper::provider::{BrowseHit, EpisodeRef};

use super::play_native_numbering::regular_episode_count;
use super::play_native_span::{spanning, Span};
use super::play_native_title_marker::EntryTitles;

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
    let span = spanning(probed, expected, &admitted, entry);
    for (row, ok) in probed.iter_mut().zip(&admitted) {
        if !ok {
            row.2 = UNFIT;
        }
    }
    if let Some(span) = span {
        apply_span(probed, expected, &span);
    }
    refused_a_fit
}

/// Scores the pool as `span` decided: the later siblings and W's
/// first half out; with an
/// own listing that fits, every other exact fit one behind it; with a
/// cut, W cut to the entry's episodes and every other exact fit one
/// behind it — an unrelated title or a spinoff sharing the franchise
/// name ("Show Side Story") is a fallback, never ahead on order.
fn apply_span(probed: &mut [Probed<'_>], expected: u32, span: &Span) {
    for &j in span.later.iter().chain(&span.firsts) {
        probed[j].2 = UNFIT;
    }
    if !span.own_fits.is_empty() || span.cut {
        for (k, row) in probed.iter_mut().enumerate() {
            if row.2 == 0 && !span.own_fits.contains(&k) {
                row.2 = 1;
            }
        }
    }
    if span.cut {
        probed[span.wide].1 = head_of(&probed[span.wide].1, expected);
        probed[span.wide].2 = 0;
    }
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
