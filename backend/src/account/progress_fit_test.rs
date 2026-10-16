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
fn the_last_episode_beside_a_larger_tracker_total_is_not_filled_or_completed() {
    // Air: Kitsu numbers 12 episodes, MAL and AniList count 13 with the
    // prologue as their episode 1. A count gap alone says nothing about
    // which episodes the tracker has that Kitsu lacks — a prologue, or
    // recaps a long show lists as 0.5s — so the watched number goes as
    // it is, and the tracker is not told the show is complete when it
    // counts episodes the user is not credited with. Filling the gap
    // waits on per-show evidence.
    assert_eq!(fit_to_tracker_total(finale(12), Some(13)), episode(12));
    // Haruhi (2009) on AniList: 28, the whole series in story order.
    assert_eq!(fit_to_tracker_total(finale(14), Some(28)), episode(14));
}

#[test]
fn the_last_episode_sends_its_own_number_when_the_totals_agree() {
    assert_eq!(fit_to_tracker_total(finale(12), Some(12)), finale(12));
}

#[test]
fn the_last_episode_never_sends_more_than_a_smaller_tracker_total() {
    // Ranma ½: Kitsu's 161-episode show maps to an 18-episode MAL entry.
    // Clamped, and — the totals differing — not completed either.
    assert_eq!(fit_to_tracker_total(finale(161), Some(18)), episode(18));
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
fn a_row_already_completed_one_short_of_a_larger_total_is_left_alone() {
    // A row an earlier finale wrote at Kitsu's 12 of the tracker's 13:
    // nothing advances, and its status is the user's.
    assert_eq!(
        reconcile_monotonic(
            fit_to_tracker_total(finale(12), Some(13)),
            row(ListStatus::Completed, 12)
        ),
        None
    );
}

#[test]
fn a_rewatch_finale_beside_a_larger_total_sends_its_number_and_stays_a_rewatch() {
    assert_eq!(
        reconcile_monotonic(
            fit_to_tracker_total(finale(12), Some(13)),
            row(ListStatus::Rewatching, 11)
        ),
        Some(episode(12))
    );
}

#[test]
fn a_rewatch_finale_at_equal_totals_stays_a_rewatch() {
    assert_eq!(
        reconcile_monotonic(
            fit_to_tracker_total(finale(12), Some(12)),
            row(ListStatus::Rewatching, 11)
        ),
        Some(episode(12))
    );
}

#[test]
fn a_finale_beside_a_larger_total_adds_an_unlisted_show_as_watching() {
    assert_eq!(
        reconcile_monotonic(fit_to_tracker_total(finale(12), Some(13)), None),
        Some(EntryUpdate {
            status: Some(ListStatus::Watching),
            progress_episodes: Some(12),
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
        // Completed survives only where the totals agree.
        let expected = if status == Some(ListStatus::Completed) && ep != total {
            None
        } else {
            status
        };
        proptest::prop_assert_eq!(fitted.status, expected);
    }

    /// The finale is never filled: it sends its own number, clamped,
    /// and completes only where the tracker's total is that number.
    #[test]
    fn the_finale_is_its_own_number_and_completes_only_at_equal_totals(
        ep in 0u32..2000,
        total in 1u32..2000,
    ) {
        let fitted = fit_to_tracker_total(finale(ep), Some(total));
        let expected = if ep == total { finale(ep) } else { episode(ep.min(total)) };
        proptest::prop_assert_eq!(fitted, expected);
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

#[test]
fn a_finale_beside_a_larger_total_on_a_row_complete_at_that_total_writes_nothing() {
    assert_eq!(
        reconcile_monotonic(
            fit_to_tracker_total(finale(12), Some(13)),
            row(ListStatus::Completed, 13)
        ),
        None
    );
}
