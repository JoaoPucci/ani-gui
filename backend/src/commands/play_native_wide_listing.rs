//! Fitting a probed pool to the entry it is picked for, by the
//! entry's own titles — split from `play_native` for the per-file
//! complexity bar.
//!
//! When the provider's search leaves the entry's own listing out of a
//! pool, its sequel can sit within the count tolerance and a year of
//! the entry ("My Star: Season 2" for `[Oshi no Ko]`). A candidate
//! whose title the entry's titles do not admit
//! ([`EntryTitles::admits`]) is still probed — it is evidence about
//! the pool — but it can never be picked.

use crate::scraper::provider::{BrowseHit, EpisodeRef};

use super::play_native_title_marker::EntryTitles;

/// The distance a candidate that may not be picked is scored at:
/// beyond every tolerance, so it never wins on count and never sets
/// the pool's best distance.
pub(super) const UNFIT: u32 = u32::MAX;

/// A probed candidate as the pick scores it: the hit, its listing,
/// its count distance from the entry, and whether its own year
/// matched the entry's.
type Probed<'h> = (&'h BrowseHit, Vec<EpisodeRef>, u32, bool);

/// Fit the probed pool to the entry: a candidate the entry's titles
/// do not admit is scored [`UNFIT`].
///
/// Candidates keep their listings, years and places, so a rule that
/// reads the pool as parts of one entry still sees every part.
pub(super) fn fit_to_entry(probed: &mut [Probed<'_>], _expected: u32, entry: EntryTitles<'_>) {
    for row in probed.iter_mut() {
        if !entry.admits(&row.0.title) {
            row.2 = UNFIT;
        }
    }
}

#[cfg(test)]
#[path = "play_native_wide_listing_test.rs"]
mod tests;
