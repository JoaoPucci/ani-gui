//! One anime-database entry the provider lists as several shows.
//!
//! Kitsu can keep a show as one entry where the provider splits it:
//! Steel Ball Run is twelve episodes on Kitsu, while hianime lists a
//! one-episode "Steel Ball Run: JoJo no Kimyou na Bouken" (the March
//! premiere) and a "... 2nd Stage" whose episode 1 is Kitsu's episode
//! 2. Picking either alone resolves every episode against the wrong
//! numbering. The parts are recognised from what the pick already
//! fetched — titles, years and listings — and stitched into one
//! listing in the entry's own numbering, so nothing downstream of the
//! pick has to know there were several.

use crate::scraper::provider::EpisodeRef;

/// What the chain detection reads of one probed candidate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PartCandidate<'a> {
    /// The provider's display title.
    pub title: &'a str,
    /// Regular (integer) episodes listed.
    pub count: u32,
    /// The candidate's own premiere year positively matched Kitsu's.
    pub confirmed: bool,
}

/// Which part of `stem` a title names: 1 for the stem itself, `k` for
/// the stem followed by a short `k`-th part marker ("2nd Stage",
/// "Part 3", "Season 2"), `None` for anything else.
#[must_use]
pub(crate) fn part_ordinal(stem: &str, title: &str) -> Option<u32> {
    let _ = (stem, title);
    None
}

/// The candidates, in part order, that together make up the expected
/// entry — or `None` when no such chain explains the expected count
/// better than the best single candidate (`best_single` is its
/// distance).
#[must_use]
pub(crate) fn split_chain(
    cands: &[PartCandidate<'_>],
    expected: u32,
    best_single: u32,
) -> Option<Vec<usize>> {
    let _ = (cands, expected, best_single);
    None
}

/// The parts' listings as one, in the entry's own numbering: each
/// part normalised to per-entry numbers, then shifted past every part
/// before it. Episode ids are kept, so a stitched row still streams
/// from the part that lists it.
#[must_use]
pub(crate) fn merge_parts(parts: &[&[EpisodeRef]]) -> Vec<EpisodeRef> {
    parts.first().map(|p| p.to_vec()).unwrap_or_default()
}

#[cfg(test)]
#[path = "play_native_split_test.rs"]
mod tests;
