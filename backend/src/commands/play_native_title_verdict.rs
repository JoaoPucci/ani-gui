//! What a pool the entry's titles narrowed answers — split from
//! `play_native_wide_listing` for the per-file complexity bar: which
//! candidate an airing-part rescue may take, the countless pick's
//! admitted head, and the verdict on a pool nothing fit.

use crate::scraper::provider::BrowseHit;

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

/// The countless pick's head narrowed to the hits the entry's titles
/// admit — the only identity a pick without a count has.
///
/// # Errors
/// [`refused_by_title`] when the titles refuse every hit.
pub(super) fn admitted_head<'h>(
    head: &[(&'h BrowseHit, bool)],
    entry: EntryTitles<'_>,
) -> crate::error::Result<Vec<(&'h BrowseHit, bool)>> {
    let admitted: Vec<_> = head
        .iter()
        .copied()
        .filter(|(h, _)| entry.admits(&h.title))
        .collect();
    if admitted.is_empty() {
        return Err(refused_by_title());
    }
    Ok(admitted)
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
