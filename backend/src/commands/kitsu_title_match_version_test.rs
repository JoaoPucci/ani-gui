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

fn state(history: std::path::PathBuf) -> AppState {
    AppState {
        history_path: history,
        ..super::title_match_tests::state()
    }
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
