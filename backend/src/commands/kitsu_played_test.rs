//! A guess never overwrites a mapping it could not tell apart from a
//! play's.

use super::*;
use crate::commands::kitsu::{allmanga_kitsu_put, watched_at_put};

/// A cache that cannot say when the mapping was written cannot say the
/// mapping is a guess: the guess fails rather than replacing it.
#[test]
fn a_mapping_whose_moment_cannot_be_read_is_not_replaced() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = crate::commands::play::tests::state_with_proxy_origin();
    state.history_path = tmp.path().join("history");
    crate::history::upsert_and_write(
        &state.history_path,
        crate::history::HistoryEntry {
            ep_no: "1".into(),
            id: "the-show-77".into(),
            title: "The Show".into(),
            watched_at: None,
            kitsu_id: None,
        },
    )
    .unwrap();
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    watched_at_put(&state, "the-show-77", now).unwrap();
    allmanga_kitsu_put(&state, "the-show-77", "21").unwrap();
    // The moment no longer reads as a number; the row is still there to
    // be replaced.
    state
        .cache_pool
        .get()
        .unwrap()
        .execute(
            "UPDATE meta_cache SET fetched_at = 'unreadable' WHERE key = ?1",
            [allmanga_kitsu_key("the-show-77")],
        )
        .unwrap();

    assert!(store_guess(&state, "the-show-77", "12").is_err());

    let body: String = state
        .cache_pool
        .get()
        .unwrap()
        .query_row(
            "SELECT body FROM meta_cache WHERE key = ?1",
            [allmanga_kitsu_key("the-show-77")],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(body, "21", "the play's mapping stands");
}
