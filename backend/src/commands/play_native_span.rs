//! Which broad listing a probed pool holds beside the entry, and what
//! the spanning-cut decision table (`docs/title-resolution.md`) decides
//! about it — split from `play_native_wide_listing`, which scores the
//! pool as decided, for the per-file complexity bar.

use super::play_native_numbering::regular_episode_count;
use super::play_native_title_marker::{stem, EntryTitles};
use super::play_native_wide_listing::Probed;

/// What the spanning-cut decision table (title-resolution.md) decided
/// beside one broad listing.
pub(super) struct Span {
    /// The broad listing W.
    pub(super) wide: usize,
    /// The siblings completing W that do not read as the entry — the
    /// next entry, never picked.
    pub(super) later: Vec<usize>,
    /// Beside a W the entry is not the head of, the listings that open
    /// past W's stem on a first division alone — W's first half, never
    /// the entry ([`EntryTitles::first_part_of`]).
    pub(super) firsts: Vec<usize>,
    /// Every candidate that is the entry's own beside W
    /// ([`EntryTitles::names_own_part`]) and fits exactly.
    pub(super) own_fits: Vec<usize>,
    /// Whether W is cut: the entry is its head, a later sibling
    /// completes it, and no own listing fits.
    pub(super) cut: bool,
}

/// The broad listing W the decision table reads, and what it decides.
/// W is admitted, carries the entry's own year, lists more than the
/// entry has and has a stem; it is read only when a sibling completes
/// it — one listing exactly the remainder, naming right after W's stem
/// a later part than the entry's ([`EntryTitles::names_later_part`])
/// and not reading as the entry — or an own listing fits beside it,
/// or, the entry not being its head, a listing opens past its stem on
/// a first division (W's first half).
pub(super) fn spanning(
    probed: &[Probed<'_>],
    expected: u32,
    admitted: &[bool],
    entry: EntryTitles<'_>,
) -> Option<Span> {
    let counts: Vec<u32> = probed
        .iter()
        .map(|(_, eps, _, _)| regular_episode_count(eps))
        .collect();
    (0..probed.len()).find_map(|m| {
        let (h, _, _, confirmed) = &probed[m];
        if !admitted[m] || !confirmed || counts[m] <= expected || stem(&h.title).is_empty() {
            return None;
        }
        let own = |k: usize| entry.names_own_part(&probed[k].0.title, &h.title);
        let head = entry.heads(&h.title);
        let own_fits: Vec<usize> = (0..probed.len())
            .filter(|&k| k != m && admitted[k] && probed[k].2 == 0 && own(k))
            .collect();
        // A sibling completing W is never an own listing that fits
        // (an O). Beside a W the entry does not head, one that is the
        // entry's own by title is not one either, whatever its count;
        // beside a W the entry heads, it is the next entry, the kind
        // set aside ("Show Part 2" beside "Show Season 2" for "Show 2").
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
        Some(Span {
            wide: m,
            later,
            firsts,
            own_fits,
            cut,
        })
    })
}
