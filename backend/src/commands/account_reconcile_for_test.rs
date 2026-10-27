//! `reconcile_for`: the monotonic reconcile against the row as each
//! tracker keeps it, closing and counting a MAL rewatch the write
//! completes.

use super::{reconcile_for, reconcile_monotonic};
use crate::account::provider::{CurrentEntry, EntryUpdate, ProviderKind};
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

fn status() -> impl Strategy<Value = ListStatus> {
    proptest::sample::select(STATUSES.to_vec())
}

fn update() -> impl Strategy<Value = EntryUpdate> {
    (
        proptest::option::of(status()),
        proptest::option::of(0u32..30),
        proptest::option::of(0u8..=100),
        proptest::option::of(0u32..10),
    )
        .prop_map(
            |(status, progress_episodes, score_0_to_100, repeat_count)| EntryUpdate {
                status,
                progress_episodes,
                score_0_to_100,
                repeat_count,
            },
        )
}

fn row() -> impl Strategy<Value = Option<CurrentEntry>> {
    proptest::option::of(
        (
            status(),
            0u32..30,
            proptest::option::of(prop_oneof![0u32..10, Just(u32::MAX)]),
        )
            .prop_map(
                |(status, progress_episodes, times_rewatched)| CurrentEntry {
                    status,
                    progress_episodes,
                    times_rewatched,
                },
            ),
    )
}

fn rewatching_row() -> impl Strategy<Value = Option<CurrentEntry>> {
    (
        0u32..30,
        proptest::option::of(prop_oneof![0u32..10, Just(u32::MAX)]),
    )
        .prop_map(|(progress_episodes, times_rewatched)| {
            Some(CurrentEntry {
                status: ListStatus::Rewatching,
                progress_episodes,
                times_rewatched,
            })
        })
}

fn closes_a_mal_rewatch(row: Option<CurrentEntry>, out: Option<&EntryUpdate>) -> bool {
    row.is_some_and(|r| r.status == ListStatus::Rewatching)
        && out.is_some_and(|u| u.status == Some(ListStatus::Completed))
}

proptest! {
    /// AniList keeps a rewatch in a status of its own, which it closes
    /// itself: its rows are reconciled exactly as before.
    #[test]
    fn anilist_is_reconciled_as_before(u in update(), r in row()) {
        prop_assert_eq!(
            reconcile_for(ProviderKind::AniList, u.clone(), r),
            reconcile_monotonic(u, r)
        );
    }

    /// A MAL row not being rewatched is reconciled as before too.
    #[test]
    fn a_mal_row_not_being_rewatched_is_reconciled_as_before(u in update(), r in row()) {
        prop_assume!(r.is_none_or(|r| r.status != ListStatus::Rewatching));
        prop_assert_eq!(
            reconcile_for(ProviderKind::MyAnimeList, u.clone(), r),
            reconcile_monotonic(u, r)
        );
    }

    /// On a MAL rewatch the write is what a Watching row would get, and
    /// where that completes the row the count of finished rewatches is
    /// the one read plus one — never lower, and never invented when the
    /// read had none. Anything short of completing leaves the count as
    /// the write had it.
    #[test]
    fn a_mal_rewatch_completed_is_counted_from_the_read(u in update(), r in rewatching_row()) {
        let kept = r.map(|r| CurrentEntry { status: ListStatus::Watching, ..r });
        let got = reconcile_for(ProviderKind::MyAnimeList, u.clone(), r);
        let base = reconcile_monotonic(u, kept);
        prop_assert_eq!(got.is_some(), base.is_some());
        let (Some(got), Some(base)) = (got, base) else { return Ok(()) };
        prop_assert_eq!(got.status, base.status);
        prop_assert_eq!(got.progress_episodes, base.progress_episodes);
        prop_assert_eq!(got.score_0_to_100, base.score_0_to_100);
        if closes_a_mal_rewatch(r, Some(&got)) {
            let read = r.and_then(|r| r.times_rewatched);
            prop_assert_eq!(got.repeat_count, read.map(|n| n.saturating_add(1)));
            if let (Some(sent), Some(read)) = (got.repeat_count, read) {
                prop_assert!(sent >= read);
            }
        } else {
            prop_assert_eq!(got.repeat_count, base.repeat_count);
        }
    }

    /// The count moves only where a MAL rewatch is closed: every other
    /// write carries the count it came with.
    #[test]
    fn the_count_moves_only_where_a_mal_rewatch_is_closed(u in update(), r in row()) {
        let got = reconcile_for(ProviderKind::MyAnimeList, u.clone(), r);
        if got.as_ref().is_some_and(|g| g.repeat_count != u.repeat_count) {
            prop_assert!(closes_a_mal_rewatch(r, got.as_ref()));
        }
    }
}
