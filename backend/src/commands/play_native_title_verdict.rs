//! What a pool the entry's titles narrowed answers — split from
//! `play_native_wide_listing` for the per-file complexity bar: which
//! candidate an airing-part rescue may take, the bounded head the
//! pick probes, and the verdict on a pool nothing fit.

use crate::scraper::provider::BrowseHit;

use super::play_native::MAX_PROBED_CANDIDATES;
use super::play_native_numbering::regular_episode_count;
use super::play_native_title_marker::EntryTitles;
use super::play_native_wide_listing::{Probed, UNFIT};

/// Whether a probed candidate may be rescued as an airing part: its
/// own year matched the entry's, its titles were admitted, and it
/// lists fewer episodes than the entry will have.
pub(super) fn rescuable(row: &Probed<'_>, expected: u32) -> bool {
    let (_, eps, distance, confirmed) = row;
    *confirmed && *distance != UNFIT && regular_episode_count(eps) < expected
}

/// The hits the pick considers, in the provider's order: up to
/// [`MAX_PROBED_CANDIDATES`] the entry's titles admit, so listings
/// named for other seasons never crowd the entry's own out of the
/// bounded head. With a count, the refused hits among the first
/// [`MAX_PROBED_CANDIDATES`] are kept too — they are probed as
/// evidence (a span's later part, a split's parts) and never picked.
/// Without one they are dropped: the titles are the only identity.
///
/// # Errors
/// [`refused_by_title`] when hits there were and the titles refused
/// them all without a count to probe them for.
pub(super) fn probe_head(
    hits: Vec<BrowseHit>,
    counted: bool,
    entry: EntryTitles<'_>,
) -> crate::error::Result<Vec<BrowseHit>> {
    let any = !hits.is_empty();
    let mut admitted = 0;
    let head: Vec<BrowseHit> = hits
        .into_iter()
        .enumerate()
        .filter(|(pos, h)| {
            if entry.admits(&h.title) {
                admitted += 1;
                admitted <= MAX_PROBED_CANDIDATES
            } else {
                counted && *pos < MAX_PROBED_CANDIDATES
            }
        })
        .map(|(_, h)| h)
        .collect();
    if any && head.is_empty() && !counted {
        return Err(refused_by_title());
    }
    Ok(head)
}

/// The verdict on a pool no candidate fit: weather when a probe died
/// unheard — the right show may have been the dead one — the
/// [`refused_by_title`] dead end when the titles refused a candidate
/// the count accepted, and the clean miss only when every candidate
/// answered and the count refused them all.
pub(super) fn rejection(
    any_transport_failure: bool,
    refused_a_fit: bool,
) -> crate::error::AniError {
    if any_transport_failure {
        crate::error::AniError::Network
    } else if refused_a_fit {
        refused_by_title()
    } else {
        crate::error::AniError::NoResults
    }
}

/// The verdict on a pool rejected because the entry's titles refused
/// a candidate the count accepted, or without a count refused every
/// candidate. Refusing by title is an inference from how titles are
/// written, and a wrong one must not be persisted as the show's
/// absence: the verdict is the answered dead end the walk moves on
/// from without a clean miss — never weather, which would open the
/// breaker on a provider that answered.
pub(super) fn refused_by_title() -> crate::error::AniError {
    crate::error::AniError::Upstream { status: 404 }
}
