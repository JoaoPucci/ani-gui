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
/// [`MAX_PROBED_CANDIDATES`] that could be picked — admitted, and not
/// another entry of the same show (the entry before this one, or the
/// season before it) — so listings named for other seasons never
/// crowd the entry's own out of the bounded head. With a count, the
/// refused hits and the other entries among the first
/// [`MAX_PROBED_CANDIDATES`] are kept too — probed as evidence (a
/// span's later part, a split's parts, a broad listing) and never
/// picked. Without one they are dropped: the titles are the only
/// identity, and another entry is no refusal. Alongside the head,
/// whether a refused hit was dropped — a countless pick that then
/// finds nothing was narrowed by the title inference, and answers
/// [`refused_by_title`] ([`countless_miss`]).
///
/// # Errors
/// [`refused_by_title`] when, without a count to probe them for, the
/// titles left nothing and refused at least one hit.
pub(super) fn probe_head(
    hits: Vec<BrowseHit>,
    counted: bool,
    entry: EntryTitles<'_>,
) -> crate::error::Result<(Vec<BrowseHit>, bool)> {
    let mut admitted = 0;
    let mut refused_dropped = false;
    let head: Vec<BrowseHit> = hits
        .into_iter()
        .enumerate()
        .filter(|(pos, h)| {
            // Another entry of the same show
            // ([`EntryTitles::another_entry`]) is never picked, so it is
            // never charged to the five: with a count it is evidence as
            // a refused hit is, without one it is dropped — but as no
            // refusal, so it leaves the miss clean.
            let admits = entry.admits(&h.title);
            if admits && !entry.another_entry(&h.title) {
                admitted += 1;
                admitted <= MAX_PROBED_CANDIDATES
            } else {
                refused_dropped |= !counted && !admits;
                counted && *pos < MAX_PROBED_CANDIDATES
            }
        })
        .map(|(_, h)| h)
        .collect();
    if head.is_empty() && refused_dropped {
        return Err(refused_by_title());
    }
    Ok((head, refused_dropped))
}

/// What a countless pick that found nothing answers: the plain miss,
/// unless the titles narrowed its head ([`probe_head`]) — then a
/// refused hit may have been the entry's listing, and the miss is the
/// [`refused_by_title`] dead end the availability cache never stores.
pub(super) fn countless_miss(
    result: crate::error::Result<super::play_native::PickedShow>,
    refused_dropped: bool,
) -> crate::error::Result<super::play_native::PickedShow> {
    match result {
        Err(crate::error::AniError::NoResults) if refused_dropped => Err(refused_by_title()),
        other => other,
    }
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
