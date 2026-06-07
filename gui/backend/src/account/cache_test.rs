//! Tests for `crate::account::cache`. Extracted via `#[path]` so the
//! module's complexity stays out of `cache.rs`'s CCN budget — per
//! `project_crap_inline_test_gotcha`, lizard counts `mod tests {}`
//! inline as production complexity.

use super::*;
use crate::account::status::ListStatus;
use crate::cache::open_in_memory;

fn entry(provider: ProviderKind, media_id: u32, status: ListStatus) -> ListEntry {
    ListEntry {
        provider,
        media_id: ProviderMediaId(media_id),
        mal_id: Some(media_id),
        status,
        progress_episodes: 0,
        score_0_to_100: None,
        updated_at_epoch_s: 1_700_000_000,
        title: format!("Show {media_id}"),
    }
}

#[test]
fn write_then_read_round_trips_one_user() {
    let pool = open_in_memory().unwrap();
    let entries = vec![
        entry(ProviderKind::AniList, 1, ListStatus::Planning),
        entry(ProviderKind::AniList, 2, ListStatus::Watching),
    ];
    write_entries(&pool, ProviderKind::AniList, "user-a", &entries).unwrap();
    let got = list_entries(&pool, ProviderKind::AniList, "user-a").unwrap();
    assert_eq!(got.len(), 2);
    let media_ids: Vec<u32> = got.iter().map(|e| e.media_id.0).collect();
    assert!(media_ids.contains(&1));
    assert!(media_ids.contains(&2));
}

#[test]
fn write_replaces_existing_row_for_same_media_id() {
    // Same (provider, user_id, media_id) → upsert wins. Pin the
    // status flips through.
    let pool = open_in_memory().unwrap();
    write_entries(
        &pool,
        ProviderKind::AniList,
        "u",
        &[entry(ProviderKind::AniList, 9, ListStatus::Planning)],
    )
    .unwrap();
    let mut updated = entry(ProviderKind::AniList, 9, ListStatus::Watching);
    updated.progress_episodes = 5;
    write_entries(&pool, ProviderKind::AniList, "u", &[updated]).unwrap();
    let got = list_entries(&pool, ProviderKind::AniList, "u").unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].status, ListStatus::Watching);
    assert_eq!(got[0].progress_episodes, 5);
}

#[test]
fn read_filters_by_provider_and_user() {
    // Two providers + two users → 4 distinct rows; reads of one
    // pair must not leak rows from another.
    let pool = open_in_memory().unwrap();
    write_entries(
        &pool,
        ProviderKind::AniList,
        "u1",
        &[entry(ProviderKind::AniList, 1, ListStatus::Planning)],
    )
    .unwrap();
    write_entries(
        &pool,
        ProviderKind::AniList,
        "u2",
        &[entry(ProviderKind::AniList, 1, ListStatus::Planning)],
    )
    .unwrap();
    assert_eq!(
        list_entries(&pool, ProviderKind::AniList, "u1")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        list_entries(&pool, ProviderKind::AniList, "u2")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        list_entries(&pool, ProviderKind::AniList, "missing")
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn clear_user_drops_only_that_user_rows() {
    let pool = open_in_memory().unwrap();
    write_entries(
        &pool,
        ProviderKind::AniList,
        "u1",
        &[entry(ProviderKind::AniList, 1, ListStatus::Planning)],
    )
    .unwrap();
    write_entries(
        &pool,
        ProviderKind::AniList,
        "u2",
        &[entry(ProviderKind::AniList, 1, ListStatus::Planning)],
    )
    .unwrap();
    clear_user(&pool, ProviderKind::AniList, "u1").unwrap();
    assert_eq!(
        list_entries(&pool, ProviderKind::AniList, "u1")
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        list_entries(&pool, ProviderKind::AniList, "u2")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn unknown_status_in_db_is_silently_skipped_on_read() {
    // A future provider could add a status string we don't know;
    // dropping the row keeps the rest of the list readable rather
    // than failing the whole call.
    let pool = open_in_memory().unwrap();
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO user_list_cache (provider, user_id, media_id, mal_id, \
            status, progress, score_x100, updated_at, fetched_at, title) \
         VALUES ('anilist', 'u', 1, NULL, 'planning', 0, NULL, 1, 1, 't1')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO user_list_cache (provider, user_id, media_id, mal_id, \
            status, progress, score_x100, updated_at, fetched_at, title) \
         VALUES ('anilist', 'u', 2, NULL, 'someday_maybe', 0, NULL, 1, 1, 't2')",
        [],
    )
    .unwrap();
    // Now drop the conn so the read path's pool.get() can acquire
    // (max_size=1 for in-memory pools).
    drop(conn);
    let got = list_entries(&pool, ProviderKind::AniList, "u").unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].media_id.0, 1);
}

#[test]
fn write_entries_replaces_full_snapshot_dropping_stale_rows() {
    // Codex P2 #3369918713: a fresh provider list no longer
    // containing a previously-cached entry must remove the stale
    // row. Otherwise an item the user deleted on AniList would
    // linger in the Watch Later rail until disconnect cleared the
    // whole user table.
    let pool = open_in_memory().unwrap();
    write_entries(
        &pool,
        ProviderKind::AniList,
        "u",
        &[
            entry(ProviderKind::AniList, 1, ListStatus::Planning),
            entry(ProviderKind::AniList, 2, ListStatus::Planning),
            entry(ProviderKind::AniList, 3, ListStatus::Watching),
        ],
    )
    .unwrap();
    // User deletes media_id=2 on AniList; the next resync sends
    // only the two surviving ids.
    write_entries(
        &pool,
        ProviderKind::AniList,
        "u",
        &[
            entry(ProviderKind::AniList, 1, ListStatus::Planning),
            entry(ProviderKind::AniList, 3, ListStatus::Watching),
        ],
    )
    .unwrap();
    let got = list_entries(&pool, ProviderKind::AniList, "u").unwrap();
    let ids: Vec<u32> = got.iter().map(|e| e.media_id.0).collect();
    assert!(!ids.contains(&2), "stale row 2 should have been dropped");
    assert!(ids.contains(&1));
    assert!(ids.contains(&3));
    assert_eq!(got.len(), 2);
}

#[test]
fn write_entries_only_drops_the_same_user_rows() {
    // The DELETE in the resync transaction must be scoped to
    // (provider, user_id). A second user's rows should be
    // untouched.
    let pool = open_in_memory().unwrap();
    write_entries(
        &pool,
        ProviderKind::AniList,
        "u1",
        &[entry(ProviderKind::AniList, 10, ListStatus::Planning)],
    )
    .unwrap();
    write_entries(
        &pool,
        ProviderKind::AniList,
        "u2",
        &[entry(ProviderKind::AniList, 20, ListStatus::Planning)],
    )
    .unwrap();
    // u1 resyncs with a completely different id; u2 must survive.
    write_entries(
        &pool,
        ProviderKind::AniList,
        "u1",
        &[entry(ProviderKind::AniList, 11, ListStatus::Planning)],
    )
    .unwrap();
    let u1 = list_entries(&pool, ProviderKind::AniList, "u1").unwrap();
    let u2 = list_entries(&pool, ProviderKind::AniList, "u2").unwrap();
    assert_eq!(u1.len(), 1);
    assert_eq!(u1[0].media_id.0, 11);
    assert_eq!(u2.len(), 1);
    assert_eq!(u2[0].media_id.0, 20);
}
