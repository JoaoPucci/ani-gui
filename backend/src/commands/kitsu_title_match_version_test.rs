//! Title matches the version before stored: not read, and still taken
//! by a removal of the show.
//!
//! A Continue row's title match is the guess's pick, stored so the next
//! load skips the search. The pick now prefers a hit whose title is the
//! row's, or whose year is the one the row names, so a match the version
//! before stored may be a pick this one would not make — Black Clover's
//! one-episode Jump Festa special for the TV series. Those rows are not
//! read; the next load picks again. A removal of the show still takes
//! them, as it takes every row it left in the cache.

use super::*;
use crate::history::HistoryEntry;
use crate::scraper::provider::ProviderId;

fn state(history: std::path::PathBuf) -> crate::scratch::Scratch<AppState> {
    let mut state = super::title_match_tests::state();
    state.history_path = history;
    state
}

const BEFORE: &str = "title-match:v4:hianime:black clover:c1";

#[test]
fn a_title_match_the_version_before_stored_is_not_read() {
    let td = tempfile::tempdir().expect("td");
    let s = state(td.path().join("history"));
    crate::cache::meta_cache_put(&s.cache_pool, BEFORE, "12625", 86_400).expect("put");
    assert_eq!(
        title_match_get(&s, Some(ProviderId::Hianime), "black clover", 1).expect("get"),
        None
    );
}

/// A match stored before the row's own episode and title ruled hits
/// out may be one of the hits they rule out: hianime's "Jujutsu Kaisen
/// (TV)" took the New Year's Special. It is not read; the next load
/// picks again.
#[test]
fn a_title_match_stored_before_the_row_ruled_hits_out_is_not_read() {
    let td = tempfile::tempdir().expect("td");
    let s = state(td.path().join("history"));
    let before = "title-match:v5:hianime:jujutsu kaisen (tv):c1";
    crate::cache::meta_cache_put(&s.cache_pool, before, "43886", 86_400).expect("put");
    assert_eq!(
        title_match_get(&s, Some(ProviderId::Hianime), "jujutsu kaisen (tv)", 1).expect("get"),
        None
    );
}

/// A match stored before the row's search could be refuted as a
/// whole may be one of the hits it refutes: hianime's "One Piece: The
/// Movie 14 - Stampede" took One Piece Movie 1. A stored match is
/// checked alone, by the rules that do not read a search as a whole, so
/// it is not read; the next load picks again.
#[test]
fn a_title_match_stored_before_a_search_could_be_refuted_is_not_read() {
    let td = tempfile::tempdir().expect("td");
    let s = state(td.path().join("history"));
    let before = "title-match:v6:hianime:one piece: the movie 14 - stampede:c1";
    crate::cache::meta_cache_put(&s.cache_pool, before, "421", 86_400).expect("put");
    assert_eq!(
        title_match_get(
            &s,
            Some(ProviderId::Hianime),
            "one piece: the movie 14 - stampede",
            1
        )
        .expect("get"),
        None
    );
}

#[test]
fn removing_the_show_takes_the_title_matches_the_version_before_stored() {
    let td = tempfile::tempdir().expect("td");
    let path = td.path().join("history");
    let s = state(path.clone());
    crate::history::write_atomic(
        &path,
        &[HistoryEntry {
            ep_no: "1".into(),
            id: "hianime:black-clover-949".into(),
            title: "Black Clover".into(),
            watched_at: None,
            kitsu_id: None,
            own_number: true,
        }],
    )
    .expect("history");
    crate::cache::meta_cache_put(&s.cache_pool, BEFORE, "12625", 86_400).expect("put");
    assert!(
        crate::commands::history::history_delete(&s, "hianime:black-clover-949").expect("delete")
    );
    assert_eq!(
        crate::cache::meta_cache_get(&s.cache_pool, BEFORE).expect("read"),
        None
    );
}

/// The version before files a match for a row that names a year under
/// its title keyed by that year — the provider lists the 2000 JoJo as
/// "JoJo's Bizarre Adventure" under `jojos-bizarre-adventure-2000-5182`.
/// A removal of the row takes it too.
#[test]
fn removing_a_show_that_names_a_year_takes_the_match_the_version_before_keyed_by_it() {
    let td = tempfile::tempdir().expect("td");
    let path = td.path().join("history");
    let s = state(path.clone());
    crate::history::write_atomic(
        &path,
        &[HistoryEntry {
            ep_no: "1".into(),
            id: "hianime:jojos-bizarre-adventure-2000-5182".into(),
            title: "JoJo's Bizarre Adventure".into(),
            watched_at: None,
            kitsu_id: None,
            own_number: true,
        }],
    )
    .expect("history");
    let keyed = "title-match:v4:hianime:jojo's bizarre adventure (2000):c1";
    crate::cache::families::undeclared_allowed(|| {
        crate::cache::meta_cache_put(&s.cache_pool, keyed, "1240", 86_400).expect("put")
    });
    assert!(crate::commands::history::history_delete(
        &s,
        "hianime:jojos-bizarre-adventure-2000-5182"
    )
    .expect("delete"));
    assert_eq!(
        crate::cache::families::undeclared_allowed(|| crate::cache::meta_cache_get(
            &s.cache_pool,
            keyed
        ))
        .expect("read"),
        None
    );
}
