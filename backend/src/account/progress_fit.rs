//! Fit a mark-watched progress write to the tracker's own episode total.
//!
//! The app numbers a show's episodes by Kitsu's list; a tracker can count
//! the same show differently. This is the seam where the count the
//! write-back sends is translated into the tracker's numbering, before
//! [`crate::commands::account::reconcile_monotonic`] keeps it monotonic.

use crate::account::provider::EntryUpdate;

/// The `update` to send to a tracker whose total for the media is
/// `tracker_episodes`.
#[must_use]
pub fn fit_to_tracker_total(update: EntryUpdate, _tracker_episodes: Option<u32>) -> EntryUpdate {
    update
}

#[cfg(test)]
#[path = "progress_fit_test.rs"]
mod tests;
