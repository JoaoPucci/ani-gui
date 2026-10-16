//! Fit a mark-watched progress write to the tracker's own episode total.
//!
//! The app numbers a show's episodes by Kitsu's list; a tracker can count
//! the same show differently. This is the seam where the count the
//! write-back sends is translated into the tracker's numbering, before
//! [`crate::commands::account::reconcile_monotonic`] keeps it monotonic.

use crate::account::provider::EntryUpdate;
use crate::account::status::ListStatus;

/// The `update` to send to a tracker whose total for the media is
/// `tracker_episodes`.
///
/// On the progress route a `Completed` status is the fan-out's signal
/// that the user finished Kitsu's last episode of a finished show (the
/// renderer's `watchedStatus`); nothing else sends it there. That
/// episode is the tracker's last whatever either side counts, so the
/// write carries the tracker's total. This is what completes a show
/// whose prologue the tracker counts as its episode 1 and Kitsu does
/// not (Air: 12 against 13) — the prologue itself was not watched, and
/// the tracker is told it was.
///
/// Any other episode is sent as its own number, but never past the
/// tracker's total: where the tracker's entry is shorter than Kitsu's
/// show (Ranma ½ maps 161 episodes onto an 18-episode MAL entry) the
/// count stops at the entry's end, and the monotonic reconcile then
/// drops the repeated value. A mid-show episode is not shifted by the
/// difference between the totals: which end the extra episode sits at
/// is not known, and the finale is the one place where it does not
/// matter.
///
/// An unknown total — AniList's `null`, MAL's `0` — leaves the write
/// as the fan-out sent it.
#[must_use]
pub fn fit_to_tracker_total(mut update: EntryUpdate, tracker_episodes: Option<u32>) -> EntryUpdate {
    let (Some(total), Some(progress)) = (
        tracker_episodes.filter(|t| *t > 0),
        update.progress_episodes,
    ) else {
        return update;
    };
    let finale = update.status == Some(ListStatus::Completed);
    update.progress_episodes = Some(if finale { total } else { progress.min(total) });
    update
}

#[cfg(test)]
#[path = "progress_fit_test.rs"]
mod tests;
