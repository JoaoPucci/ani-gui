//! A list row as each tracker keeps it for the write-back's reconcile.

use super::{CurrentEntry, ProviderKind};
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
    ) {
        let row = CurrentEntry { status, progress_episodes: progress };
        let kept = row.as_kept_by(kind);
        prop_assert_eq!(kept.progress_episodes, progress);
        if kind == ProviderKind::MyAnimeList && status == ListStatus::Rewatching {
            prop_assert_eq!(kept.status, ListStatus::Watching);
        } else {
            prop_assert_eq!(kept, row);
        }
    }
}
