//! Tests for the single-row cache write-through. Moved here with
//! `upsert_entry` so `cache.rs` stays under the CRAP ceiling.

use super::*;
use crate::account::cache::{list_entries, write_entries};
use crate::account::provider::{ListEntry, ProviderMediaId};
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
fn upsert_entry_updates_one_row_without_touching_others() {
    // Codex P2 #3412673593: after a tracker write, the single updated
    // entry must be written back so the Watch Later rail (status ===
    // 'planning' filter) drops a just-started show. upsert_entry touches
    // only its own (provider, user_id, media_id) row — unlike
    // write_entries, it must NOT wipe the rest of the list.
    let pool = open_in_memory().unwrap();
    write_entries(
        &pool,
        ProviderKind::AniList,
        "u",
        &[
            entry(ProviderKind::AniList, 1, ListStatus::Planning),
            entry(ProviderKind::AniList, 2, ListStatus::Planning),
        ],
    )
    .unwrap();
    let mut started = entry(ProviderKind::AniList, 1, ListStatus::Watching);
    started.progress_episodes = 3;
    upsert_entry(&pool, ProviderKind::AniList, "u", &started).unwrap();

    let got = list_entries(&pool, ProviderKind::AniList, "u").unwrap();
    assert_eq!(got.len(), 2, "the sibling planning row must survive");
    let row1 = got.iter().find(|e| e.media_id.0 == 1).unwrap();
    assert_eq!(row1.status, ListStatus::Watching);
    assert_eq!(row1.progress_episodes, 3);
    let row2 = got.iter().find(|e| e.media_id.0 == 2).unwrap();
    assert_eq!(row2.status, ListStatus::Planning);
}

#[test]
fn upsert_entry_does_not_regress_progress() {
    // Codex P2 #3416732383: two mark-watched writes for the same show can
    // race the cache write-through (it runs outside push_progress's
    // per-show lock). The upsert must be monotonic — a stale, lower-
    // progress write landing last must NOT overwrite the newer row.
    let pool = open_in_memory().unwrap();
    let mut newer = entry(ProviderKind::AniList, 1, ListStatus::Watching);
    newer.progress_episodes = 6;
    upsert_entry(&pool, ProviderKind::AniList, "u", &newer).unwrap();

    let mut older = entry(ProviderKind::AniList, 1, ListStatus::Watching);
    older.progress_episodes = 5;
    upsert_entry(&pool, ProviderKind::AniList, "u", &older).unwrap();

    let got = list_entries(&pool, ProviderKind::AniList, "u").unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(
        got[0].progress_episodes, 6,
        "a stale lower-progress write must not regress the cache"
    );
}

#[test]
fn upsert_entry_force_overwrites_lower_progress() {
    // The explicit detail-page editor lets the user correct an
    // over-count downward (e.g. 6 → 3). That write must land in the
    // cache so the rail/editor reflect the corrected value — the
    // monotonic guard on `upsert_entry` would swallow it, so the
    // explicit path uses `upsert_entry_force`, which overwrites
    // unconditionally.
    let pool = open_in_memory().unwrap();
    let mut newer = entry(ProviderKind::AniList, 1, ListStatus::Watching);
    newer.progress_episodes = 6;
    upsert_entry(&pool, ProviderKind::AniList, "u", &newer).unwrap();

    let mut corrected = entry(ProviderKind::AniList, 1, ListStatus::Watching);
    corrected.progress_episodes = 3;
    upsert_entry_force(&pool, ProviderKind::AniList, "u", &corrected).unwrap();

    let got = list_entries(&pool, ProviderKind::AniList, "u").unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(
        got[0].progress_episodes, 3,
        "an explicit downward edit must overwrite the cache"
    );
}

#[test]
fn upsert_entry_rejects_a_write_older_than_the_cached_row() {
    // Codex P2 #3423044438: a mark-watched cache write-through runs
    // outside push_progress's per-show lock, so a stale one (left the
    // provider earlier) can land AFTER an explicit downward correction.
    // Its higher progress would pass the monotonic guard and clobber the
    // correction. Guard on updated_at recency too: a write stamped older
    // than the cached row loses, so the explicit edit survives.
    let pool = open_in_memory().unwrap();
    // Explicit correction: progress 3, freshly stamped (T2).
    let mut corrected = entry(ProviderKind::AniList, 1, ListStatus::Watching);
    corrected.progress_episodes = 3;
    corrected.updated_at_epoch_s = 2_000;
    upsert_entry_force(&pool, ProviderKind::AniList, "u", &corrected).unwrap();

    // Stale mark-watched write-through: higher progress 6 but an OLDER
    // tracker timestamp (T1 < T2) — it left the provider before the edit.
    let mut stale = entry(ProviderKind::AniList, 1, ListStatus::Watching);
    stale.progress_episodes = 6;
    stale.updated_at_epoch_s = 1_000;
    upsert_entry(&pool, ProviderKind::AniList, "u", &stale).unwrap();

    let got = list_entries(&pool, ProviderKind::AniList, "u").unwrap();
    assert_eq!(
        got[0].progress_episodes, 3,
        "a stale older write must not clobber the newer explicit correction"
    );
}

#[test]
fn upsert_entry_inserts_when_absent() {
    let pool = open_in_memory().unwrap();
    upsert_entry(
        &pool,
        ProviderKind::AniList,
        "u",
        &entry(ProviderKind::AniList, 7, ListStatus::Watching),
    )
    .unwrap();
    let got = list_entries(&pool, ProviderKind::AniList, "u").unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].media_id.0, 7);
}
