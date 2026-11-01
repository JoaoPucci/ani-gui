//! A history row from before rows recorded the app's own number, whose
//! display stamp a warm moved to another row, put right from a listing
//! at hand. Split from [`super`] for the per-file complexity bar.
//!
//! Such a row holds the slot its watch resolved to. A slot whose tag is
//! not its slot reads as its tag only while the show's one display
//! stamp names it, and builds before the marker stamped every resolve
//! of such a row — warms included — so a warm of another left this one
//! reading as its slot: one episode high past a single recap. Read
//! against a listing that agrees with the sidecar, the slot is the
//! episode its tag says, and the stamp is put back on it.

use super::numbering_offset_store::{rewrite, Row};
use super::{AppState, Numbering};
use crate::scraper::provider::EpisodeRef;

#[path = "numbering_offset_drift_rule.rs"]
mod rule;
use rule::restamp;
#[cfg(test)]
use rule::Restamp;

/// Put the display stamp of the show `slug` keys back on its legacy
/// history row's slot, where [`restamp`] finds `listing` agrees with
/// the sidecar.
/// Decided again under the store's locks, against the stamp as it
/// stands there; a show with no stamp, no legacy row, or a history file
/// that cannot be read is not written.
pub(super) fn put_drift_right(
    state: &AppState,
    slug: &str,
    listing: &[EpisodeRef],
    unread_past: Option<u32>,
) {
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
    if restamp(
        &row.ep_no,
        numbering.offset(slug),
        &stamp,
        listing,
        unread_past,
    )
    .is_none()
    {
        return;
    }
    let change = |rows: &mut Vec<Row>| {
        let Some(store_row) = rows.iter_mut().find(|r| r.slug == slug) else {
            return;
        };
        let Some(stamp) = store_row.display.as_ref() else {
            return;
        };
        if let Some(fix) = restamp(&row.ep_no, store_row.offset, stamp, listing, unread_past) {
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
