//! Which broad listing a probed pool holds beside the entry, and what
//! the spanning-cut decision table (`docs/title-resolution.md`) decides
//! about it — split from `play_native_wide_listing`, which scores the
//! pool as decided, for the per-file complexity bar.

use super::play_native_numbering::regular_episode_count;
use super::play_native_title_marker::{stem, EntryTitles};
use super::play_native_wide_listing::Probed;

#[path = "play_native_span_beside.rs"]
mod span_beside;
use span_beside::beside;

/// What the spanning-cut decision table (title-resolution.md) decided
/// beside one broad listing.
pub(super) struct Span {
    /// The broad listing W.
    pub(super) wide: usize,
    /// The siblings completing W — the next entry, never picked. None
    /// is an own listing that fits; beside a W the entry does not head,
    /// none is the entry's own by title either, while beside one it
    /// heads a sibling reading as the entry but not fitting is one.
    pub(super) later: Vec<usize>,
    /// Beside a W the entry is not the head of, the listings that open
    /// past W's stem on a first division alone — W's first half, never
    /// the entry ([`EntryTitles::first_part_of`]).
    pub(super) firsts: Vec<usize>,
    /// The listings that name a season before the one W carries — the
    /// season before W ([`EntryTitles::names_earlier_season`]), never
    /// the entry.
    pub(super) earlier: Vec<usize>,
    /// Every candidate that is the entry's own beside W
    /// ([`EntryTitles::names_own_part`]) and fits exactly.
    pub(super) own_fits: Vec<usize>,
    /// Whether W is cut: the entry is its head, a later sibling
    /// completes it, and no own listing fits.
    pub(super) cut: bool,
}

/// The broad listing W the decision table reads, and what it decides:
/// the first the entry heads beside which a completing sibling or an
/// own listing exists, and only when there is none, the first beside
/// which any row applies.
/// W is admitted, carries the entry's own year, lists more than the
/// entry has and has a stem; it is read only when a sibling completes
/// it — one listing exactly the remainder, naming right after W's stem
/// a division later than W ([`EntryTitles::names_later_part`]),
/// and no own listing that fits — nor, beside a W the entry does not
/// head, the entry's own by title — or an own listing fits beside it,
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
    let spans: Vec<(Span, bool)> = (0..probed.len())
        .filter_map(|m| beside(probed, &counts, m, expected, admitted, entry))
        .collect();
    // The W the entry heads, beside a completing sibling or an own
    // listing, wherever it sits; only without one, the first W any row
    // applies to.
    let headed = spans
        .iter()
        .position(|(span, head)| *head && !(span.later.is_empty() && span.own_fits.is_empty()));
    let pick = headed.unwrap_or(0);
    spans.into_iter().nth(pick).map(|(span, _)| span)
}
