//! A mapping is a play's when a play stored it, and only then: never
//! read from when it was written.

use super::*;
use crate::commands::kitsu::{
    allmanga_kitsu_delete, allmanga_kitsu_put, allmanga_kitsu_put_played, watched_at_put,
};

fn listed_state(tmp: &tempfile::TempDir) -> AppState {
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
    state
}

fn now_ms() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

fn mapped(state: &AppState) -> Option<String> {
    crate::commands::kitsu::allmanga_kitsu_get(state, "the-show-77").unwrap()
}

/// A play's mapping is the show the user played: a guess leaves it
/// standing, whenever the play stored it.
#[test]
fn a_mapping_a_play_stored_is_not_replaced_by_a_guess() {
    let tmp = tempfile::tempdir().unwrap();
    let state = listed_state(&tmp);
    allmanga_kitsu_put_played(&state, "the-show-77", "21").unwrap();

    assert!(mapping_played(&state, "the-show-77").unwrap());
    store_guess(&state, "the-show-77", "12").unwrap();
    assert_eq!(
        mapped(&state).as_deref(),
        Some("21"),
        "the play's mapping stands"
    );
}

/// A Continue load can store a guess for a row whose watch recorded
/// no id, and a play can stamp the show within the same second. The
/// guess sits beside the stamp, and is still a guess. So is a mapping
/// written before plays marked theirs: which one stored it cannot be
/// told, and it is read the way that cannot pin a guess.
#[test]
fn a_guess_beside_the_watch_stamp_is_not_played() {
    let tmp = tempfile::tempdir().unwrap();
    let state = listed_state(&tmp);
    store_guess(&state, "the-show-77", "12").unwrap();
    watched_at_put(&state, "the-show-77", now_ms()).unwrap();

    assert!(!mapping_played(&state, "the-show-77").unwrap());
    store_guess(&state, "the-show-77", "13").unwrap();
    assert_eq!(
        mapped(&state).as_deref(),
        Some("13"),
        "a guess replaces a guess"
    );
}

/// A play's mark names the mapping it stored. A guess that replaces
/// the mapping after it was dropped is not vouched for by the mark the
/// dropped one left, even naming the same entry.
#[test]
fn a_dropped_mapping_takes_its_play_mark() {
    let tmp = tempfile::tempdir().unwrap();
    let state = listed_state(&tmp);
    watched_at_put(&state, "the-show-77", now_ms()).unwrap();
    allmanga_kitsu_put_played(&state, "the-show-77", "21").unwrap();
    allmanga_kitsu_delete(&state, "the-show-77").unwrap();
    allmanga_kitsu_put(&state, "the-show-77", "21").unwrap();

    assert!(!mapping_played(&state, "the-show-77").unwrap());
}

/// Removing the show takes the mark with the mapping; so does clearing
/// the history.
#[test]
fn a_removal_takes_the_play_mark() {
    let tmp = tempfile::tempdir().unwrap();
    let state = listed_state(&tmp);
    watched_at_put(&state, "the-show-77", now_ms()).unwrap();
    allmanga_kitsu_put_played(&state, "the-show-77", "21").unwrap();
    crate::commands::history_forget::forget_finders(&state, "the-show-77", "The Show").unwrap();
    allmanga_kitsu_put(&state, "the-show-77", "21").unwrap();
    assert!(!mapping_played(&state, "the-show-77").unwrap(), "removed");

    watched_at_put(&state, "the-show-77", now_ms()).unwrap();
    allmanga_kitsu_put_played(&state, "the-show-77", "21").unwrap();
    crate::commands::history_forget::forget_all(&state).unwrap();
    watched_at_put(&state, "the-show-77", now_ms()).unwrap();
    allmanga_kitsu_put(&state, "the-show-77", "21").unwrap();
    assert!(!mapping_played(&state, "the-show-77").unwrap(), "cleared");
}
