//! What a listing makes of a show's numbering stamps, and which of two
//! repairs' writes stands: the pure half of [`super`]. Split from it
//! for the per-file complexity bar.

use super::super::numbering_offset_store::Row;
use crate::commands::listing_tags::{Hearing, Order};
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

/// The repairs this run took of one show, each its listing's hearing
/// and what the listing made of the stamps from before them.
pub(super) type Repairs = Vec<(Hearing, Stamps)>;

/// The repairs of `repairs` no other one was heard wholly after: the
/// ones that still have a say.
pub(super) fn outstanding(repairs: &[(Hearing, Stamps)]) -> Repairs {
    repairs
        .iter()
        .filter(|(heard, _)| {
            !repairs
                .iter()
                .any(|(other, _)| heard.against(Some(*other)) == Order::Earlier)
        })
        .cloned()
        .collect()
}

/// The stamps `repairs` leave, from `base`, the stamps before them: the
/// ones the repairs no other was heard after ([`outstanding`]) agree on.
/// Those cannot be ordered among themselves, so a former tag only some
/// of them name is not named, as another's listing may give it to a row
/// of its own, and where they put the display stamp in different places
/// it stays as it was. The order the repairs were taken in plays no part.
#[must_use]
pub(super) fn standing(base: &Stamps, repairs: &[(Hearing, Stamps)]) -> Stamps {
    let last = outstanding(repairs);
    let Some((_, first)) = last.first() else {
        return base.clone();
    };
    if last.iter().any(|(_, s)| s.display != first.display) {
        return base.clone();
    }
    Stamps {
        display: first.display.clone(),
        former: first
            .former
            .clone()
            .filter(|_| last.iter().all(|(_, s)| s.former == first.former)),
    }
}
