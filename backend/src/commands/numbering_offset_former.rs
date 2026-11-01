//! Display stamps an earlier version wrote with a row's former tag —
//! the tag a stitched listing gave a row before it was numbered as it
//! is now — and putting them right. Split from [`super`] for the
//! per-file complexity bar.

#[cfg(test)]
use super::display_stamp;
use super::repair::{repair, repair_carried, Carrier, Listing};
use super::AppState;
use crate::commands::listing_tags::Hearing;

#[path = "numbering_offset_former_read.rs"]
mod read;
pub use read::{former_stamp, stamp_beside};

#[path = "numbering_offset_former_drop.rs"]
mod drop;
use drop::drop_shared_former;
#[cfg(test)]
use drop::kept_former;

/// Put a display stamp an earlier version wrote right: when the slug's
/// stamp names one of `former_tags`' slots with the tag that version
/// gave it, it names the tag the row has now, and remembers the one it
/// replaced unless a row of the listing carries it now (`shared`): what
/// is kept under it may be that row's. Any other stamp is left alone,
/// and a show whose stamp names no such row is not written. A former
/// tag a repair kept, which a row of this listing carries now, is
/// dropped (`drop_shared_former`). Decided
/// under the store's locks, against the row as it stands there, and
/// ordered against nothing: a walk's repair, which knows when its
/// listing was heard, goes through [`retag_from_listing`] or
/// [`retag_from_resolve`].
pub fn retag_display(
    state: &AppState,
    slug: &str,
    former_tags: &[(u32, String, String)],
    shared: &dyn Fn(&str) -> bool,
) {
    let listing = Listing {
        former_tags,
        shared,
        rows: None,
    };
    repair(state, slug, &listing, None);
    drop_shared_former(state, slug, shared);
}

/// [`retag_display`] from a resolve — a play's, a warm's, a download's —
/// for the show it resolved, and, from the same listing, a stamp a warm
/// moved off the show's legacy history row back on it, where the listing
/// agrees with the sidecar: one repair, ordered by when the resolve
/// heard its listing against another walk's ([`super::repair`]).
pub fn retag_from_resolve(
    state: &AppState,
    native: &crate::commands::play_native_resolve::NativeResolved,
) {
    // A row carries a former tag where the listing's extra tags carry it
    // in the entry's numbering (`names_a_row_now`).
    let carrier = Carrier::Entry {
        extra: native.extra_tags.clone(),
        offset: native.numbering_offset,
    };
    let shared = |former: &str| carrier.carries(former);
    let listing = Listing {
        former_tags: &native.former_tags,
        shared: &shared,
        rows: Some((&native.listing, native.unread_past)),
    };
    repair_carried(state, &native.slug, &listing, native.heard, &carrier);
    // The pick's other versions: each copy's history row reads a stamp
    // of its own, put right from that copy's listing.
    for (slug, episodes) in &native.other_copies {
        retag_from_listing(state, slug, episodes, native.unread_past, native.heard);
    }
}

/// [`retag_from_resolve`] from a listing at hand — a probe's, a
/// download's — for the show `slug` keys, heard as `heard` says.
/// `unread_past` is the pick's
/// ([`crate::commands::play_native::PickedShow::unread_past`]): no row
/// past it gets a stamp. The listing's record is the walk's to write,
/// at its pick ([`crate::commands::listing_tags`]): a repair runs once
/// the walk is done, by when a newer walk may have recorded a newer
/// listing.
pub fn retag_from_listing(
    state: &AppState,
    slug: &str,
    episodes: &[crate::scraper::provider::EpisodeRef],
    unread_past: Option<u32>,
    heard: Option<Hearing>,
) {
    let carrier = Carrier::Tags(episodes.iter().filter_map(|e| e.number2.clone()).collect());
    let shared = |former: &str| carrier.carries(former);
    let former_tags = crate::commands::play_native_merge::former_tags_of(episodes);
    let listing = Listing {
        former_tags: &former_tags,
        shared: &shared,
        rows: Some((episodes, unread_past)),
    };
    repair_carried(state, slug, &listing, heard, &carrier);
}

#[cfg(test)]
#[path = "numbering_offset_former_test.rs"]
mod tests;

#[cfg(test)]
#[path = "numbering_offset_heard_test.rs"]
mod heard_tests;
