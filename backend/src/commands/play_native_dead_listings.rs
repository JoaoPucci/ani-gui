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
/// refused, by the titles or by that rule, and the titles of every dead
/// listing that could not have won: with a count such a hit is probed
/// or kept as evidence, and as a broad listing's completing sibling it
/// decides the pick. Every title is read past a bracketed version tag,
/// as every reader of a probed candidate's divisions reads. A dead
/// listing that is another entry of the same show counts for none:
/// answered, it would have been scored out.
#[derive(Default)]
pub(super) struct DeadListings {
    pub(super) could_win: bool,
    pub(super) refused: bool,
    siblings: Vec<String>,
}

impl DeadListings {
    /// Note a candidate titled `title` whose listing answered
    /// not-found; `could_win` is what the name rules say of it.
    pub(super) fn note(&mut self, entry: EntryTitles<'_>, title: &str, could_win: bool) {
        // Read past a version tag, as every reader of a probed
        // candidate's divisions reads: the listing was probed.
        let admitted = entry.admits_as_read(title);
        let another = admitted && entry.another_entry_as_read(title);
        let stands = could_win && admitted;
        self.could_win |= stands && !another;
        self.refused |= !(another || stands);
        if !stands {
            self.siblings
                .push(super::play_native_version_copies::without_version_tag(
                    title,
                ));
        }
    }

    /// Whether the pool's dead listings may have decided its pick: one
    /// could have won, or one that could not would, had it answered,
    /// have been the completing sibling of a broad listing the span table
    /// cuts — a live candidate still in the running, from the entry's
    /// year, listing more episodes than the entry, headed by the entry,
    /// of which the dead hit names a later part.
    pub(super) fn might_decide(
        &self,
        probed: &[Probed<'_>],
        expected: u32,
        entry: EntryTitles<'_>,
    ) -> bool {
        self.could_win
            || probed.iter().any(|(w, eps, d, confirmed)| {
                // The broad listing read past its version tag, as the
                // spanning cut reads it and as the siblings were noted.
                let wide = super::play_native_version_copies::without_version_tag(&w.title);
                *confirmed
                    && *d != UNFIT
                    && regular_episode_count(eps) > expected
                    && entry.names(&wide)
                    && self
                        .siblings
                        .iter()
                        .any(|sibling| entry.names_later_part(sibling, &wide))
            })
    }

    /// Whether the pool refused a hit: a live one the count accepted
    /// (`refused_a_fit`), or a dead one the titles or the name rule
    /// refuse — a refused hit the pool holds whatever its count, since
    /// nothing it holds could be the entry.
    pub(super) fn refused_or(&self, refused_a_fit: bool) -> bool {
        refused_a_fit || self.refused
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
