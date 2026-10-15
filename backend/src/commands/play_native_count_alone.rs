//! A count with no year is no identity — split from
//! `play_native_titled` for the per-file complexity bar.
//!
//! An entry Kitsu counts but does not date has one signal the pick
//! can score a hit by, and a count fits many shows: the provider's
//! search answers a title it does not list with a page of other
//! shows, and among thirty of them one within the tolerance is the
//! rule rather than the exception. Of the fourteen such entries in
//! Kitsu's to-be-announced catalogue on 2026-10-15, seven were played
//! as another show on their count alone — a two-episode hentai OVA for
//! a five-episode idol OVA, a twelve-episode romance for a twelve-
//! episode donghua — and the two the provider does list are both
//! listed under a name one of the entry's titles carries.
//!
//! So for such an entry the count alone vouches for nothing: a probed
//! candidate is picked only when it is the title searched or the entry
//! names it, the rule the countless pick and the single-video pick
//! already apply. A dated entry is not read here at all — its year
//! vouches, and fourteen of the recorded dated entries are picked by
//! their year and count under a name none of their titles carries —
//! nor is an entry one video long, whose rule is
//! [`super::play_native_single_video`]'s.

use super::play_native::ep_count_threshold;
use super::play_native_title_grammar::normalized;
use super::play_native_title_marker::EntryTitles;
use super::play_native_wide_listing::{Probed, UNFIT};

/// Whether the entry's count is all that could vouch for a hit: a
/// count above one is known and no premiere year is. With a year, a
/// hit's own page can vouch for it; for a count of one, the
/// single-video rule reads the pool.
#[must_use]
pub(crate) fn count_stands_alone(expected: u32, year: Option<u32>) -> bool {
    expected != 1 && year.is_none()
}

/// For an entry whose count stands alone ([`count_stands_alone`]),
/// scores [`UNFIT`] every probed candidate that carries no identity
/// beyond its count: not the title searched, and not named as one of
/// the entry's titles ([`EntryTitles::names`]). Every other entry is
/// left exactly as it is.
///
/// Returns whether a candidate the count alone would have accepted
/// was scored out: the refusal is an inference from how the candidate
/// is named, so a pool rejected after it is
/// [`super::play_native_title_verdict::refused_by_title`], not a clean
/// miss.
pub(super) fn unvouched_counted(
    probed: &mut [Probed<'_>],
    expected: u32,
    needle: &str,
    entry: EntryTitles<'_>,
    year: Option<u32>,
) -> bool {
    if !count_stands_alone(expected, year) {
        return false;
    }
    let tolerance = ep_count_threshold(expected);
    let mut refused_a_fit = false;
    for (h, _, distance, confirmed) in probed.iter_mut() {
        let vouched = *confirmed || normalized(&h.title) == needle || entry.names(&h.title);
        if vouched || *distance == UNFIT {
            continue;
        }
        refused_a_fit |= *distance <= tolerance;
        *distance = UNFIT;
    }
    refused_a_fit
}

#[cfg(test)]
#[path = "play_native_count_alone_test.rs"]
mod tests;
