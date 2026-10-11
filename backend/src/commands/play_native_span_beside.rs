//! What the spanning-cut decision table decides beside one candidate
//! read as the broad listing — split from `play_native_span` for the
//! per-file complexity bar.

use super::*;

/// What the decision table decides beside the candidate `m` read as W,
/// and whether the entry heads it; `None` when `m` is no W or no row
/// applies beside it.
pub(super) fn beside(
    probed: &[Probed<'_>],
    counts: &[u32],
    m: usize,
    expected: u32,
    admitted: &[bool],
    entry: EntryTitles<'_>,
) -> Option<(Span, bool)> {
    let (h, _, _, confirmed) = &probed[m];
    if !admitted[m] || !confirmed || counts[m] <= expected || stem(&h.title).is_empty() {
        return None;
    }
    let own = |k: usize| entry.names_own_part(&probed[k].0.title, &h.title);
    let head = entry.heads(&h.title);
    let own_fits: Vec<usize> = (0..probed.len())
        .filter(|&k| k != m && admitted[k] && probed[k].2 == 0 && own(k))
        .collect();
    // A sibling completing W is never an own listing that fits (an
    // O). Beside a W the entry does not head, one that is the entry's
    // own by title is not one either, whatever its count; beside a W
    // the entry heads, it is the next entry, the kind set aside ("Show
    // Part 2" beside "Show Season 2" for "Show 2").
    let later: Vec<usize> = (0..probed.len())
        .filter(|&j| {
            j != m
                && counts[j] == counts[m] - expected
                && entry.names_later_part(&probed[j].0.title, &h.title)
                && !own_fits.contains(&j)
                && (head || !own(j))
        })
        .collect();
    let firsts: Vec<usize> = (0..probed.len())
        .filter(|&k| !head && k != m && entry.first_part_of(&probed[k].0.title, &h.title))
        .collect();
    if later.is_empty() && own_fits.is_empty() && firsts.is_empty() {
        return None;
    }
    let cut = head && !later.is_empty() && own_fits.is_empty();
    let span = Span {
        wide: m,
        later,
        firsts,
        own_fits,
        cut,
    };
    Some((span, head))
}
