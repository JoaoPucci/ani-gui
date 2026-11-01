//! What a listing makes of a show's numbering stamps, and which of two
//! repairs' writes stands: the pure half of [`super`]. Split from it
//! for the per-file complexity bar.

use super::super::numbering_offset_store::Row;
use crate::commands::listing_tags::Order;
use crate::scraper::provider::EpisodeRef;

/// A show's numbering stamps: the display stamp, and the (slot, tag) a
/// repair replaced ([`Row::former`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Stamps {
    pub(super) display: Option<(u32, String)>,
    pub(super) former: Option<(u32, String)>,
}

impl Stamps {
    pub(super) fn of(row: &Row) -> Self {
        Self {
            display: row.display.clone(),
            former: row.former.clone(),
        }
    }
}

/// What a repair reads off a listing at hand.
pub(in super::super) struct Listing<'a> {
    /// The listing's rows an earlier version tagged otherwise: (slot,
    /// the tag then, the tag now).
    pub(in super::super) former_tags: &'a [(u32, String, String)],
    /// Whether a former tag is one a row of the listing carries now.
    pub(in super::super) shared: &'a dyn Fn(&str) -> bool,
    /// The listing and its pick's `unread_past`, for putting a stamp a
    /// warm moved back on the show's legacy history row
    /// ([`super::super::drift`]); `None` leaves that step out.
    pub(in super::super) rows: Option<(&'a [EpisodeRef], Option<u32>)>,
}

/// The stamps `listing` makes of `from`, for a show stamped with
/// `offset` whose legacy history row holds `row_ep`: a display stamp
/// that names a slot by the tag an earlier version gave it names it by
/// its tag now, remembering the one it replaced where no row carries it
/// now; then a stamp a warm moved off the legacy row goes back on it,
/// where the listing agrees with the sidecar.
pub(super) fn repaired(
    from: &Stamps,
    offset: u32,
    row_ep: Option<&str>,
    listing: &Listing<'_>,
) -> Stamps {
    let mut stamps = from.clone();
    if let Some((stamped, tag)) = &from.display {
        if let Some((slot, former, now)) = listing
            .former_tags
            .iter()
            .find(|(slot, former, _)| slot == stamped && former == tag)
        {
            stamps.display = Some((*slot, now.clone()));
            stamps.former = (!(listing.shared)(former)).then(|| (*slot, former.clone()));
        }
    }
    if let (Some((rows, unread_past)), Some(row_ep), Some(stamp)) =
        (listing.rows, row_ep, stamps.display.as_ref())
    {
        if let Some(fix) = super::super::drift::restamp(row_ep, offset, stamp, rows, unread_past) {
            stamps.display = Some(fix.display);
            stamps.former = fix.former;
        }
    }
    stamps
}

/// The stamps a repair writes, or `None` to write nothing, given how
/// its hearing stands against the last one this run took (`order`),
/// the stamps now (`current`), the stamps before this run's first
/// repair (`base`), and what the repair's listing makes of those
/// (`ours`, [`repaired`] from `base`).
#[must_use]
pub(super) fn settle(
    order: Order,
    current: &Stamps,
    base: &Stamps,
    ours: Stamps,
) -> Option<Stamps> {
    let next = match order {
        Order::Earlier => return None,
        // A listing that repairs nothing leaves the last repair as it is.
        _ if ours == *base => return None,
        Order::Later => ours,
        // Overlapping a repair this run took, yet the stamps are as they
        // were before it: two overlapping repairs disagreed, and every
        // other one heard alongside them leaves them so.
        Order::Overlapping if current == base => return None,
        // The same display stamp: a former tag only one of the two names
        // may be a row's of the other's listing, and is not named.
        Order::Overlapping if ours.display == current.display => Stamps {
            former: ours.former.filter(|f| current.former.as_ref() == Some(f)),
            display: ours.display,
        },
        // Two listings that put the stamp in different places, neither
        // heard after the other: the stamp stays as it was before.
        Order::Overlapping => base.clone(),
    };
    (next != *current).then_some(next)
}
