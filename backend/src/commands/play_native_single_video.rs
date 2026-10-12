//! A single video's count is no identity — split from
//! `play_native_titled` for the per-file complexity bar.
//!
//! A movie, a special or a one-episode entry is one video long, and
//! so is every other single video a search answers with: a count of
//! one fits them all. For such an entry the count cannot tell its
//! video from a franchise's other films and specials, or from a
//! crossover that carries the franchise's name among other shows'.

use super::play_native::ep_count_threshold;
use super::play_native_title_grammar::normalized;
use super::play_native_title_marker::EntryTitles;
use super::play_native_wide_listing::{Probed, UNFIT};

/// For an entry one video long, scores [`UNFIT`] every probed
/// candidate that carries no identity beyond its count: not the title
/// searched, not named as one of the entry's titles
/// ([`EntryTitles::heads`]), and without its own year matching the
/// entry's. A pool with no count other than one is left as it is.
///
/// Returns whether a candidate the count alone would have accepted
/// was scored out: the refusal is an inference from how the candidate
/// is named, so a pool rejected after it is
/// [`super::play_native_title_verdict::refused_by_title`], not a clean
/// miss.
pub(super) fn unvouched_single_videos(
    probed: &mut [Probed<'_>],
    expected: u32,
    needle: &str,
    entry: EntryTitles<'_>,
) -> bool {
    if expected != 1 {
        return false;
    }
    let tolerance = ep_count_threshold(expected);
    let mut refused_a_fit = false;
    for (h, _, distance, confirmed) in probed.iter_mut() {
        let vouched = *confirmed || normalized(&h.title) == needle || entry.heads(&h.title);
        if vouched || *distance == UNFIT {
            continue;
        }
        refused_a_fit |= *distance <= tolerance;
        *distance = UNFIT;
    }
    refused_a_fit
}
