//! A history row from before rows recorded the app's own number, whose
//! display stamp a warm moved to another row, put right from a listing
//! at hand. Split from [`super`] for the per-file complexity bar.
//!
//! Such a row holds the slot its watch resolved to. A slot whose tag is
//! not its slot reads as its tag only while the show's one display
//! stamp names it, and builds before the marker stamped every play
//! and warm that landed on such a row, so a warm of another left this one
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
/// Decided again under the store's locks, against the stamp and the
/// history row as they stand there. A show with no stamp, no legacy
/// row, or a history file that cannot be read is left as it is.
///
/// A stamp that is not the one the unlocked read found is left as it
/// is. A build from before the marker, running beside this one on the
/// same files, stamps a play before it writes the play's row, outside
/// these locks; a stamp that moved meanwhile may have that row on its
/// way. A warm writes the same stamp with no row to follow, and the
/// next listing at hand repairs it.
///
/// A play whose stamp the repair reads as it stands (written before
/// the unlocked read, or written again with the same value since) and
/// whose row lands after the repair's last read of the history file
/// cannot be told from a warm: the stamp is put on the old row's slot
/// until the next listing at hand. See "A stamp repair can land inside
/// an older instance's play" in `docs/deferred-work.md`.
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
    let Some(ep_no) = legacy_ep_no(state, slug) else {
        return;
    };
    if restamp(&ep_no, numbering.offset(slug), &stamp, listing, unread_past).is_none() {
        return;
    }
    let change = |rows: &mut Vec<Row>| {
        let Some(store_row) = rows.iter_mut().find(|r| r.slug == slug) else {
            return;
        };
        let Some(now) = store_row.display.as_ref().filter(|now| **now == stamp) else {
            return;
        };
        let Some(ep_no) = legacy_ep_no(state, slug) else {
            return;
        };
        if let Some(fix) = restamp(&ep_no, store_row.offset, now, listing, unread_past) {
            store_row.display = Some(fix.display);
            store_row.former = fix.former;
        }
    };
    if let Err(e) = rewrite(state, change) {
        tracing::warn!(slug, error = ?e, "numbering stamp repair failed");
    }
}

/// The number the show `slug`'s history row holds, while that row is
/// from before rows recorded the app's own number. Read without the
/// history guard, so a caller inside `stamp_numbering`'s hold cannot
/// deadlock; a history file that cannot be read says nothing about
/// the row.
fn legacy_ep_no(state: &AppState, slug: &str) -> Option<String> {
    crate::history::read_all(&state.history_path)
        .ok()?
        .into_iter()
        .find(|e| e.id == slug && !e.own_number)
        .map(|e| e.ep_no)
}

#[cfg(test)]
#[path = "numbering_offset_drift_test.rs"]
mod tests;

#[cfg(test)]
#[path = "numbering_offset_drift_race_test.rs"]
mod race_tests;
