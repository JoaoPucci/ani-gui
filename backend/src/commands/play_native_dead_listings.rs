//! What a pool's dead listings — candidates whose listing answered
//! not-found — say about its verdict; split from
//! `play_native_title_verdict` for the per-file complexity bar.

use super::play_native_numbering::regular_episode_count;
use super::play_native_title_marker::EntryTitles;
use super::play_native_title_verdict::refused_by_title;
use super::play_native_wide_listing::{Probed, UNFIT};

/// What the listings that answered not-found say about a pool: whether
/// one of them could have won — the entry's titles admit it, it is not
/// another entry, and the rule that reads its name would have let it
/// stand ([`super::play_native_dated_name::may_win`]) — whether one was
/// refused, by the titles or by that rule, and whether one the titles
/// refused was among them: with a count such a hit is probed as
/// evidence, and as a broad listing's completing sibling it decides the
/// pick. A dead listing that is another entry of the same show counts
/// for none: answered, it would have been scored out.
#[derive(Default)]
pub(super) struct DeadListings {
    pub(super) could_win: bool,
    pub(super) refused: bool,
    unadmitted: bool,
}

impl DeadListings {
    /// Note a candidate titled `title` whose listing answered
    /// not-found; `could_win` is what the name rules say of it.
    pub(super) fn note(&mut self, entry: EntryTitles<'_>, title: &str, could_win: bool) {
        let admitted = entry.admits(title);
        let another = admitted && entry.another_entry(title);
        let stands = could_win && admitted;
        self.could_win |= stands && !another;
        self.refused |= !(another || stands);
        self.unadmitted |= !admitted;
    }

    /// Whether the pool's dead listings may have decided its pick: one
    /// could have won, or one the titles refused stands beside a live
    /// candidate listing more episodes than the entry has — a broad
    /// listing it may have completed.
    pub(super) fn might_decide(&self, probed: &[Probed<'_>], expected: u32) -> bool {
        self.could_win
            || (self.unadmitted
                && probed
                    .iter()
                    .any(|(_, eps, d, _)| *d != UNFIT && regular_episode_count(eps) > expected))
    }

    /// The verdict on a pool every probe of which failed: weather when
    /// any died in transport, the dead end on a listing that could have
    /// won, the refusal by name when one was refused and none could
    /// have won, and otherwise — every dead listing another entry — the
    /// clean miss the pool would have been had they answered.
    pub(super) fn verdict(&self, any_transport_failure: bool) -> crate::error::AniError {
        if any_transport_failure {
            crate::error::AniError::Network
        } else if self.could_win {
            crate::error::AniError::Upstream { status: 404 }
        } else if self.refused {
            refused_by_title()
        } else {
            crate::error::AniError::NoResults
        }
    }
}
