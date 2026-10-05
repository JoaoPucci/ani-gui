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

/// The candidates, in part order, that together make up the expected
/// entry — or `None` when no such chain explains the expected count
/// better than the best single candidate (`best_single` is its
/// distance).
///
/// Every part's own premiere year has to match Kitsu's: the parts of
/// one entry air close together, and that is what separates them from
/// a sequel season the provider names the same way. The chain is
/// accepted where it fits the expected count within the picker's
/// tolerance, or falls short of it — an airing entry, which has not
/// aired everything Kitsu counts.
/// A lead must also pass [`lead_may_stitch`].
#[must_use]
pub(crate) fn split_chain(
    cands: &[PartCandidate<'_>],
    expected: u32,
    best_single: u32,
    _entry_titles: &[&str],
) -> Option<Vec<usize>> {
    let tolerance = super::play_native::ep_count_threshold(expected);
    let single_fits = best_single <= tolerance;
    let mut best: Option<(u32, Vec<usize>)> = None;
    let leads = (0..cands.len()).filter(|&i| {
        let c = &cands[i];
        c.confirmed && c.count > 0 && lead_may_stitch(c, expected, tolerance, single_fits)
    });
    for lead in leads {
        let chain = chain_from(cands, lead);
        for len in 2..=chain.len() {
            let sum: u32 = chain[..len].iter().map(|&i| cands[i].count).sum();
            let dist = sum.abs_diff(expected);
            let fits = dist <= tolerance || sum < expected;
            let stands =
                stands_over_single(cands, &chain[1..len], expected, best_single, single_fits);
            if fits && stands && dist < best_single && best.as_ref().is_none_or(|(d, _)| dist < *d)
            {
                best = Some((dist, chain[..len].to_vec()));
            }
        }
    }
    best.map(|(_, chain)| chain)
}

/// The pick a split entry makes: the first part's hit, with every
/// part's listing stitched under it — or `None` when the probed
/// candidates are not a split entry.
pub(crate) fn stitched(
    probed: &[(
        &crate::scraper::provider::BrowseHit,
        Vec<EpisodeRef>,
        u32,
        bool,
    )],
    expected: u32,
    best_single: u32,
    entry_titles: &[&str],
) -> Option<super::play_native::PickedShow> {
    let cands: Vec<PartCandidate<'_>> = probed
        .iter()
        .map(|(h, eps, _, confirmed)| PartCandidate {
            title: &h.title,
            count: super::play_native_numbering::regular_episode_count(eps),
            confirmed: *confirmed,
            offset: super::play_native_numbering::numbering_offset(eps),
        })
        .collect();
    let chain = split_chain(&cands, expected, best_single, entry_titles)?;
    let listings: Vec<&[EpisodeRef]> = chain.iter().map(|&i| probed[i].1.as_slice()).collect();
    Some(super::play_native::PickedShow {
        hit: probed[chain[0]].0.clone(),
        episodes: merge_parts(&listings),
    })
}

#[cfg(test)]
#[path = "play_native_split_test.rs"]
mod tests;
