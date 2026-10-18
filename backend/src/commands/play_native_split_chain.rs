//! Choosing the chain of parts that explains a split entry's expected
//! count, and stitching the pick from it; split from [`super`] so each
//! file stays inside the CRAP gate's per-file bar.

use super::*;

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
/// A lead must also be admitted by the entry's titles and pass
/// [`lead_may_stitch`]. Of chains that fit equally well, one led by a
/// candidate whose part agrees with the entry's wins, then the first
/// lead in provider order.
#[must_use]
pub(crate) fn split_chain(
    cands: &[PartCandidate<'_>],
    expected: u32,
    best_single: u32,
) -> Option<Vec<usize>> {
    let tolerance = crate::commands::play_native::ep_count_threshold(expected);
    let single_fits = best_single <= tolerance;
    let mut best: Option<((u32, bool), Vec<usize>)> = None;
    let leads = (0..cands.len()).filter(|&i| {
        let c = &cands[i];
        c.admitted
            && c.confirmed
            && c.count > 0
            && lead_may_stitch(c, expected, tolerance, single_fits)
    });
    for lead in leads {
        let chain = chain_from(cands, lead);
        for len in 2..=chain.len() {
            let sum: u32 = chain[..len].iter().map(|&i| cands[i].count).sum();
            let dist = sum.abs_diff(expected);
            let fits = dist <= tolerance || sum < expected;
            let stands =
                stands_over_single(cands, &chain[1..len], expected, best_single, single_fits);
            let key = (dist, !cands[lead].agrees);
            if fits && stands && dist < best_single && best.as_ref().is_none_or(|(k, _)| key < *k) {
                best = Some((key, chain[..len].to_vec()));
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
    entry: crate::commands::play_native_title_marker::EntryTitles<'_>,
) -> Option<crate::commands::play_native::PickedShow> {
    let cands: Vec<PartCandidate<'_>> = probed
        .iter()
        .map(|(h, eps, distance, confirmed)| PartCandidate {
            title: &h.title,
            count: crate::commands::play_native_numbering::regular_episode_count(eps),
            confirmed: *confirmed,
            offset: crate::commands::play_native_numbering::numbering_offset(eps),
            admitted: *distance != crate::commands::play_native_wide_listing::UNFIT,
            agrees: entry.part_agrees(&h.title),
        })
        .collect();
    let chain = split_chain(&cands, expected, best_single)?;
    let listings: Vec<&[EpisodeRef]> = chain.iter().map(|&i| probed[i].1.as_slice()).collect();
    Some(crate::commands::play_native::PickedShow {
        hit: probed[chain[0]].0.clone(),
        episodes: merge_parts(&listings),
        stitched: true,
    })
}
