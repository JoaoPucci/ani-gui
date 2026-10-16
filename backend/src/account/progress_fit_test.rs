//! The progress a mark-watched write sends, fitted to the tracker's
//! episode total. Cases by name first, then the invariants as
//! properties over every count.

use super::fit_to_tracker_total;
use crate::account::provider::{CurrentEntry, EntryUpdate};
use crate::account::status::ListStatus;
use crate::commands::account::reconcile_monotonic;

/// What the fan-out sends for an episode that is not Kitsu's last of a
/// finished show: progress alone.
fn episode(ep: u32) -> EntryUpdate {
    EntryUpdate {
        progress_episodes: Some(ep),
        ..Default::default()
    }
}

/// What the fan-out sends for Kitsu's last episode of a finished show.
fn finale(ep: u32) -> EntryUpdate {
    EntryUpdate {
        status: Some(ListStatus::Completed),
        progress_episodes: Some(ep),
        ..Default::default()
    }
}

#[test]
fn the_last_episode_of_a_finished_show_fills_a_tracker_that_counts_a_prologue() {
    // Air: Kitsu numbers 12 episodes, MAL and AniList count 13 with the
    // prologue as their episode 1. Finishing Kitsu's 12 completes the
    // tracker's 13.
    assert_eq!(fit_to_tracker_total(finale(12), Some(13)), finale(13));
}

#[test]
fn the_last_episode_sends_its_own_number_when_the_totals_agree() {
    assert_eq!(fit_to_tracker_total(finale(12), Some(12)), finale(12));
}

#[test]
fn the_last_episode_never_sends_more_than_a_smaller_tracker_total() {
    // Ranma ½: Kitsu's 161-episode show maps to an 18-episode MAL entry.
    assert_eq!(fit_to_tracker_total(finale(161), Some(18)), finale(18));
}

#[test]
fn an_earlier_episode_is_sent_as_its_own_number_beside_a_larger_total() {
    // Not the finale (or an ongoing show): the prologue offset is not
    // guessed mid-show.
    assert_eq!(fit_to_tracker_total(episode(11), Some(13)), episode(11));
    assert_eq!(fit_to_tracker_total(episode(12), Some(13)), episode(12));
}

#[test]
fn an_episode_past_a_smaller_tracker_total_is_clamped_to_it() {
    assert_eq!(fit_to_tracker_total(episode(30), Some(18)), episode(18));
    assert_eq!(fit_to_tracker_total(episode(18), Some(18)), episode(18));
    assert_eq!(fit_to_tracker_total(episode(5), Some(18)), episode(5));
}

#[test]
fn an_unknown_tracker_total_leaves_the_write_alone() {
    for total in [None, Some(0)] {
        assert_eq!(fit_to_tracker_total(finale(12), total), finale(12));
        assert_eq!(fit_to_tracker_total(episode(30), total), episode(30));
    }
}

#[test]
fn a_write_without_progress_is_left_alone() {
    let status_only = EntryUpdate {
        status: Some(ListStatus::Completed),
        ..Default::default()
    };
    assert_eq!(
        fit_to_tracker_total(status_only.clone(), Some(13)),
        status_only
    );
}

fn row(status: ListStatus, progress_episodes: u32) -> Option<CurrentEntry> {
    Some(CurrentEntry {
        status,
        progress_episodes,
    })
}

#[test]
fn a_row_stuck_one_short_of_a_prologue_total_completes_on_the_finale() {
    // A row written before the rule, at Kitsu's 12 of the tracker's 13.
    assert_eq!(
        reconcile_monotonic(
            fit_to_tracker_total(finale(12), Some(13)),
            row(ListStatus::Completed, 12)
        ),
        Some(finale(13))
    );
}

#[test]
fn a_rewatch_finale_reaches_the_tracker_total_and_stays_a_rewatch() {
    assert_eq!(
        reconcile_monotonic(
            fit_to_tracker_total(finale(12), Some(13)),
            row(ListStatus::Rewatching, 11)
        ),
        Some(episode(13))
    );
}

#[test]
fn a_finale_on_a_row_already_complete_at_the_total_sends_no_progress() {
    assert_eq!(
        reconcile_monotonic(
            fit_to_tracker_total(finale(12), Some(13)),
            row(ListStatus::Completed, 13)
        ),
        Some(EntryUpdate {
            status: Some(ListStatus::Completed),
            ..Default::default()
        })
    );
}

#[test]
fn episodes_past_a_smaller_total_stop_at_it_without_completing() {
    assert_eq!(
        reconcile_monotonic(
            fit_to_tracker_total(episode(30), Some(18)),
            row(ListStatus::Watching, 18)
        ),
        None
    );
}

fn any_status() -> impl proptest::strategy::Strategy<Value = Option<ListStatus>> {
    proptest::prop_oneof![
        proptest::strategy::Just(None),
        proptest::strategy::Just(Some(ListStatus::Completed)),
        proptest::strategy::Just(Some(ListStatus::Watching)),
    ]
}

proptest::proptest! {
    /// Nothing sent ever exceeds a known tracker total.
    #[test]
    fn a_known_total_bounds_the_progress_sent(
        ep in 0u32..2000,
        total in 1u32..2000,
        status in any_status(),
    ) {
        let update = EntryUpdate { status, progress_episodes: Some(ep), ..Default::default() };
        let fitted = fit_to_tracker_total(update, Some(total));
        proptest::prop_assert!(fitted.progress_episodes.unwrap() <= total);
        proptest::prop_assert_eq!(fitted.status, status);
    }

    /// The finale of a finished show is the tracker's last episode,
    /// whatever either side counts.
    #[test]
    fn the_finale_sends_the_tracker_total(ep in 0u32..2000, total in 1u32..2000) {
        let fitted = fit_to_tracker_total(finale(ep), Some(total));
        proptest::prop_assert_eq!(fitted, finale(total));
    }

    /// Any other episode inside the tracker's count is sent as its own
    /// number.
    #[test]
    fn an_episode_within_the_total_is_its_own_number(
        total in 1u32..2000,
        ep_seed in 0u32..2000,
    ) {
        let ep = ep_seed % (total + 1);
        proptest::prop_assert_eq!(fit_to_tracker_total(episode(ep), Some(total)), episode(ep));
    }

    /// With the total unknown the write is exactly what the fan-out sent.
    #[test]
    fn an_unknown_total_is_the_identity(
        ep in 0u32..2000,
        status in any_status(),
        score in proptest::option::of(0u8..=100),
    ) {
        let update = EntryUpdate {
            status,
            progress_episodes: Some(ep),
            score_0_to_100: score,
            repeat_count: None,
        };
        proptest::prop_assert_eq!(fit_to_tracker_total(update.clone(), None), update.clone());
        proptest::prop_assert_eq!(fit_to_tracker_total(update.clone(), Some(0)), update);
    }

    /// Fitted then reconciled, a write never lowers the row's progress.
    #[test]
    fn a_fitted_write_never_lowers_existing_progress(
        ep in 0u32..2000,
        total in proptest::option::of(0u32..2000),
        current in 0u32..2000,
        at_finale in proptest::bool::ANY,
    ) {
        let update = if at_finale { finale(ep) } else { episode(ep) };
        let sent = reconcile_monotonic(
            fit_to_tracker_total(update, total),
            row(ListStatus::Watching, current),
        );
        if let Some(progress) = sent.and_then(|u| u.progress_episodes) {
            proptest::prop_assert!(progress > current);
        }
    }
}
