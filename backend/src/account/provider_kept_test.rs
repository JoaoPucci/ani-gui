//! A list row as each tracker keeps it for the write-back's reconcile,
//! and what counts as an update with nothing in it.

use super::{CurrentEntry, EntryUpdate, ProviderKind};
use crate::account::status::ListStatus;
use proptest::prelude::*;

const STATUSES: [ListStatus; 6] = [
    ListStatus::Planning,
    ListStatus::Watching,
    ListStatus::Completed,
    ListStatus::Paused,
    ListStatus::Dropped,
    ListStatus::Rewatching,
];
const KINDS: [ProviderKind; 3] = [
    ProviderKind::AniList,
    ProviderKind::MyAnimeList,
    ProviderKind::InHouse,
];

proptest! {
    /// Only a MAL row read as rewatching changes — to the Watching row
    /// MAL lists it as — and nothing but its status: every other row, on
    /// every tracker, is kept as it is.
    #[test]
    fn only_a_mal_rewatch_is_kept_as_watching(
        status in proptest::sample::select(STATUSES.to_vec()),
        kind in proptest::sample::select(KINDS.to_vec()),
        progress in 0u32..2000,
        times in proptest::option::of(0u32..50),
    ) {
        let row = CurrentEntry { status, progress_episodes: progress, times_rewatched: times };
        let kept = row.as_kept_by(kind);
        prop_assert_eq!(kept.progress_episodes, progress);
        prop_assert_eq!(kept.times_rewatched, times);
        if kind == ProviderKind::MyAnimeList && status == ListStatus::Rewatching {
            prop_assert_eq!(kept.status, ListStatus::Watching);
        } else {
            prop_assert_eq!(kept, row);
        }
    }

    /// An update is empty exactly when it carries no field: any one field
    /// set makes it worth sending.
    #[test]
    fn an_update_is_empty_only_with_no_field(
        status in proptest::option::of(proptest::sample::select(STATUSES.to_vec())),
        progress in proptest::option::of(0u32..2000),
        score in proptest::option::of(0u8..=100),
        repeat in proptest::option::of(0u32..50),
    ) {
        let update = EntryUpdate {
            status,
            progress_episodes: progress,
            score_0_to_100: score,
            repeat_count: repeat,
        };
        let none = status.is_none() && progress.is_none() && score.is_none() && repeat.is_none();
        prop_assert_eq!(update.is_empty(), none);
    }

    /// A row stands at a total exactly when, as the tracker keeps it, it
    /// has reached that total and is Completed or a rewatch the tracker
    /// closes itself: a MAL rewatch never does, an AniList one does.
    #[test]
    fn a_row_stands_at_a_total_only_finished_as_its_tracker_keeps_it(
        status in proptest::sample::select(STATUSES.to_vec()),
        kind in proptest::sample::select(KINDS.to_vec()),
        progress in 0u32..30,
        total in 0u32..30,
    ) {
        let row = CurrentEntry { status, progress_episodes: progress, times_rewatched: None };
        let finished = match status {
            ListStatus::Completed => true,
            ListStatus::Rewatching => kind != ProviderKind::MyAnimeList,
            _ => false,
        };
        prop_assert_eq!(row.stands_at(kind, total), progress >= total && finished);
    }
}
