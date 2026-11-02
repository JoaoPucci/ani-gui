//! A history row from before rows recorded the app's own number, whose
//! display stamp a warm moved to another row, put right from a listing
//! at hand: the rule. Split from [`super`] for the per-file complexity bar.
//!
//! Such a row holds the slot its watch resolved to. A slot whose tag is
//! not its slot reads as its tag only while the show's one display
//! stamp names it, and builds before the marker stamped every play
//! and warm that landed on such a row, so a warm of another left this one
//! reading as its slot: one episode high past a single recap. Read
//! against a listing that agrees with the sidecar, the slot is the
//! episode its tag says, and the stamp is put back on it.
//!
//! The rule is here; the repair is made with the show's other stamp
//! repairs, in the order their listings were heard ([`super::repair`]).

#[path = "numbering_offset_drift_rule.rs"]
mod rule;
#[cfg(test)]
use super::Numbering;
#[cfg(test)]
use crate::scraper::provider::EpisodeRef;
pub(super) use rule::restamp;
#[cfg(test)]
use rule::Restamp;

/// Put the display stamp of the show `slug` keys back on its legacy
/// history row's slot, where [`restamp`] finds `listing` agrees with
/// the sidecar: a repair from that listing alone, which knows no
/// hearing ([`super::repair`]). The walks repair with their listings'
/// former tags as well, and ordered by when they heard them.
#[cfg(test)]
pub(super) fn put_drift_right(
    state: &super::AppState,
    slug: &str,
    listing: &[EpisodeRef],
    unread_past: Option<u32>,
) {
    let listing = super::repair::Listing {
        former_tags: &[],
        shared: &|_| false,
        rows: Some((listing, unread_past)),
    };
    super::repair::repair(state, slug, &listing, None);
}

#[cfg(test)]
#[path = "numbering_offset_drift_test.rs"]
mod tests;

#[cfg(test)]
#[path = "numbering_offset_drift_race_test.rs"]
mod race_tests;
