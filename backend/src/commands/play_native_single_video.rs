//! A single video's count is no identity — split from
//! `play_native_titled` for the per-file complexity bar.
//!
//! A movie, a special or a one-episode entry is one video long, and
//! so is every other single video a search answers with: a count of
//! one fits them all. For such an entry the count cannot tell its
//! video from a franchise's other films and specials, or from a
//! crossover that carries the franchise's name among other shows'.

use super::play_native::ep_count_threshold;
use super::play_native_count_alone::score_out_unvouched;
use super::play_native_title_marker::EntryTitles;
use super::play_native_wide_listing::Probed;

/// For an entry one video long, scores
/// [`UNFIT`](super::play_native_wide_listing::UNFIT) every probed
/// candidate that carries no identity beyond its count: not the title
/// searched, not named as one of the entry's titles
/// ([`EntryTitles::names`]), and without its own year matching the
/// entry's. An entry whose count is not one is left as it is.
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
    score_out_unvouched(probed, ep_count_threshold(expected), needle, entry)
}

#[cfg(test)]
#[path = "play_native_single_video_test.rs"]
mod tests;
