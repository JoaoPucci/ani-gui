//! Fit a mark-watched progress write to the tracker's own episode total.
//!
//! The app numbers a show's episodes by Kitsu's list; a tracker can count
//! the same show differently. This is the seam where the count the
//! write-back sends is clamped to the tracker's total and the finale's
//! completed status is kept or dropped against it, before
//! [`crate::commands::account::reconcile_monotonic`] keeps it monotonic.

use crate::account::provider::EntryUpdate;
use crate::account::status::ListStatus;

/// The `update` to send to a tracker whose total for the media is
/// `tracker_episodes`.
///
/// The count is sent as its own number, but never past the tracker's
/// total: where the tracker's entry is shorter than Kitsu's show
/// (AniList splits Ranma ½'s 161 episodes into an 18-episode entry and
/// its sequel) the count
/// stops at the entry's end, and the monotonic reconcile then drops the
/// repeated value.
///
/// On the progress route a `Completed` status is the fan-out's signal
/// that the user finished Kitsu's last episode of a finished show (the
/// renderer's `watchedStatus`). It is kept only where the tracker's
/// total is that episode. Beside any other known total the write
/// carries progress alone: a count gap does not say what the tracker's
/// other episodes are — a prologue it numbers as episode 1 (Air on
/// AniList: 13 against Kitsu's and MyAnimeList's 12), recaps a long show lists as 0.5s, a different
/// cut — so the gap is not filled, and the tracker is not told the show
/// is complete while it counts episodes the user is not credited with.
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
    if update.status == Some(ListStatus::Completed) && progress != total {
        update.status = None;
    }
    update.progress_episodes = Some(progress.min(total));
    update
}

#[cfg(test)]
#[path = "progress_fit_test.rs"]
mod tests;
