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

use super::play_native_merge::merge_parts;
use super::play_native_part_title::part_ordinal;

#[path = "play_native_split_chain.rs"]
mod chain;
#[cfg(test)]
use chain::split_chain;
pub(crate) use chain::stitched;

/// What the chain detection reads of one probed candidate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PartCandidate<'a> {
    /// The provider's display title.
    pub title: &'a str,
    /// Regular (integer) episodes listed.
    pub count: u32,
    /// The candidate's own premiere year positively matched Kitsu's.
    pub confirmed: bool,
    /// The listing's continuation offset ([`numbering_offset`]): above
    /// zero for a part numbered cumulatively.
    ///
    /// [`numbering_offset`]: super::play_native_numbering::numbering_offset
    pub offset: u32,
}

/// The parts that follow candidate `lead`, in order, while each next
/// ordinal is present: `[lead, part 2, part 3, ...]`.
fn chain_from(cands: &[PartCandidate<'_>], lead: usize) -> Vec<usize> {
    let usable = |c: &PartCandidate<'_>| c.confirmed && c.count > 0;
    let mut chain = vec![lead];
    for k in 2.. {
        let next = cands.iter().enumerate().position(|(j, c)| {
            j != lead && usable(c) && part_ordinal(cands[lead].title, c.title) == Some(k)
        });
        match next {
            Some(j) => chain.push(j),
            None => break,
        }
    }
    chain
}

/// Whether candidate `lead` may head a chain at all.
///
/// Never when it is numbered cumulatively: played alone, its key holds
/// that offset and its history rows speak those numbers, and a stitched
/// play would restamp the key at zero and misread them.
///
/// Nor, where some single candidate fits (`single_fits`), when the lead
/// is itself one inside the tolerance: a bare title that fits is the
/// entry (a cour and its same-year sequel, 12 + 1 against 13), however
/// well the sum fits.
fn lead_may_stitch(
    lead: &PartCandidate<'_>,
    expected: u32,
    tolerance: u32,
    single_fits: bool,
) -> bool {
    lead.offset == 0 && (!single_fits || lead.count.abs_diff(expected) > tolerance)
}

/// Whether a chain prefix may be stitched over the best single
/// candidate. With none inside the tolerance there is no near miss to
/// protect. With one, the count cannot tell a finished split (1 + 11
/// against 12) from a cour and its sequel (12 + 1 against 13); which
/// candidate is the near miss can: the chain stands over it only when
/// the near miss is one of the chain's later parts.
fn stands_over_single(
    cands: &[PartCandidate<'_>],
    later: &[usize],
    expected: u32,
    best_single: u32,
    single_fits: bool,
) -> bool {
    !single_fits
        || later
            .iter()
            .any(|&i| cands[i].count.abs_diff(expected) == best_single)
}

#[cfg(test)]
#[path = "play_native_split_test.rs"]
mod tests;
