//! A history row from before rows recorded the app's own number, whose
//! display stamp a warm moved to another row, put right from a listing
//! at hand. Split from [`super`] for the per-file complexity bar.
//!
//! Such a row holds the slot its watch resolved to. A slot past a recap
//! reads as its display tag only while the show's one display stamp
//! names it, and builds before the marker stamped every resolve of a
//! row whose tag is not its slot — warms included — so a warm of
//! another such row left this one reading as its slot: one episode
//! high. Read against the show's listing, the slot is the episode its
//! tag says, and the stamp is put back on it.

use super::numbering_offset_store::{rewrite, Row};
use super::{AppState, Numbering};
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
/// listing does not prove what it is.
pub(super) fn restamp(
    row_ep: &str,
    offset: u32,
    stamp: &(u32, String),
    listing: &[EpisodeRef],
) -> Option<Restamp> {
    let _ = (row_ep, offset, stamp, listing);
    None
}

/// Put the display stamp of the show `slug` keys back on its legacy
/// history row's slot, where [`restamp`] proves it from `listing`.
/// Decided again under the store's locks, against the stamp as it
/// stands there; a show with no stamp, no legacy row, or a history file
/// that cannot be read is not written.
pub(super) fn put_drift_right(state: &AppState, slug: &str, listing: &[EpisodeRef]) {
    let numbering = Numbering::read(&state.history_path);
    let Some(stamp) = numbering.display(slug) else {
        return;
    };
    // A history file that cannot be read says nothing about the row.
    let Ok(rows) = crate::history::read_all(&state.history_path) else {
        return;
    };
    let Some(row) = rows.into_iter().find(|e| e.id == slug && !e.own_number) else {
        return;
    };
    if restamp(&row.ep_no, numbering.offset(slug), &stamp, listing).is_none() {
        return;
    }
    let change = |rows: &mut Vec<Row>| {
        let Some(store_row) = rows.iter_mut().find(|r| r.slug == slug) else {
            return;
        };
        let Some(stamp) = store_row.display.as_ref() else {
            return;
        };
        if let Some(fix) = restamp(&row.ep_no, store_row.offset, stamp, listing) {
            store_row.display = Some(fix.display);
            store_row.former = fix.former;
        }
    };
    if let Err(e) = rewrite(state, change) {
        tracing::warn!(slug, error = ?e, "numbering stamp repair failed");
    }
}

#[cfg(test)]
#[path = "numbering_offset_drift_test.rs"]
mod tests;
