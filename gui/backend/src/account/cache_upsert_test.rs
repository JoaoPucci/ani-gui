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
