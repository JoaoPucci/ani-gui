//! Whether a listing at hand agrees with the sidecar a legacy history
//! row reads through, and the display stamp the row then reads through.
//! Split from [`super`] for the per-file complexity bar.

use crate::scraper::provider::EpisodeRef;

/// What the display stamp of a show whose legacy row it no longer names
/// becomes: the row's slot with its tag now, and the tag an earlier
/// version gave that row, where it had one no row carries now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Restamp {
    pub(super) display: (u32, String),
    pub(super) former: Option<(u32, String)>,
}

/// The stamp a legacy row holding `row_ep` reads through, from
/// `listing`, given the show's stamped `offset` and the display `stamp`
/// it has now; `None` where the row reads right already, or where the
/// listing disagrees with the sidecar.
///
/// The listing agrees when it numbers the show on the stamped offset
/// and tags the stamped slot as the stamp does. Then the row's slot,
/// listed once, is read as the episode its tag says; a slot whose tag
/// is its slot reads right without a stamp, and one the stamp names
/// already reads through it. The stamp was written after the row, so a
/// re-listing ahead of the row's slot between the two is not seen.
///
/// A slot past `unread_past` — Kitsu's count, when no reading reached
/// the rows the listing runs past it — is an appended extra or a whole
/// episode, unknown which, and its tag is not kept for it.
pub(super) fn restamp(
    row_ep: &str,
    offset: u32,
    stamp: &(u32, String),
    listing: &[EpisodeRef],
    unread_past: Option<u32>,
) -> Option<Restamp> {
    use crate::commands::play_native_episode::tag_matches;
    let slot: u32 = row_ep.trim().parse().ok()?;
    let (stamped, stamped_tag) = stamp;
    if *stamped == slot
        || crate::commands::play_native_appended::past_an_unread_count(unread_past, slot)
        || crate::commands::play_native_numbering::numbering_offset(listing) != offset
    {
        return None;
    }
    let stamped_row = only_row(listing, *stamped)?;
    let shown = stamped_row
        .number2
        .clone()
        .unwrap_or_else(|| stamped_row.number.to_string());
    if !tag_matches(&shown, stamped_tag) {
        return None;
    }
    let row = only_row(listing, slot)?;
    let tag = row.number2.clone()?;
    if tag_matches(&tag, &slot.to_string()) {
        return None;
    }
    // What is kept under a former tag another row carries now may be
    // that row's.
    let shared = |former: &str| {
        listing
            .iter()
            .any(|e| e.number2.as_deref().is_some_and(|t| tag_matches(t, former)))
    };
    let former = row
        .former_tag
        .clone()
        .filter(|f| !shared(f))
        .map(|f| (slot, f));
    Some(Restamp {
        display: (slot, tag),
        former,
    })
}

/// The listing's row in `slot`, when it lists exactly one.
fn only_row(listing: &[EpisodeRef], slot: u32) -> Option<&EpisodeRef> {
    let mut rows = listing.iter().filter(|e| e.number == slot);
    let row = rows.next()?;
    rows.next().is_none().then_some(row)
}
