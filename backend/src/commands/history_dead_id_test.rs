//! A row whose recorded Kitsu id Kitsu no longer has. Kitsu can delete
//! an entry; the home page's read of the row's id then gets a 404 (or
//! 410), and the backend remembers the answer. The detail page's
//! resume lookup then takes the row as one that records no id: its
//! stored mapping when that names a live entry, and otherwise the
//! title match the home page stored for it — so the entry the row is
//! now matched to finds the row. A failure that says nothing about the
//! id marks nothing, and a later read that succeeds revives it.

use super::*;
use crate::app::AppState;
use crate::history::{write_atomic, HistoryEntry};
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use crate::scraper::provider::ShowKey;
use std::path::PathBuf;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DETAIL_FIXTURE: &[u8] =
    include_bytes!("../../../tests/fixtures/kitsu/anime_one_piece_detail.json");

const SHOW: &str = "cowboy-bebop-1";
const TITLE: &str = "Cowboy Bebop";

fn state_at(history_path: PathBuf, kitsu_base: &str) -> AppState {
    AppState {
        anidb_base: None,
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 0),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path,
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        hianime_base: None,
        hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        provider_order: vec![crate::scraper::provider::ProviderId::Anidb],
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_base),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// A history holding one row that records `recorded`, with Kitsu
/// answering `/anime/<recorded>` with `status`.
async fn row_recording(recorded: &str, status: u16) -> (tempfile::TempDir, MockServer, AppState) {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/anime/{recorded}")))
        .respond_with(ResponseTemplate::new(status))
        .mount(&mock)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let history = tmp.path().join("history");
    write_atomic(
        &history,
        &[HistoryEntry {
            ep_no: "5".into(),
            id: SHOW.into(),
            title: TITLE.into(),
            watched_at: None,
            kitsu_id: Some(recorded.into()),
        }],
    )
    .unwrap();
    let state = state_at(history, &mock.uri());
    (tmp, mock, state)
}

fn found(state: &AppState, kitsu_id: &str) -> Option<String> {
    history_by_kitsu(state, kitsu_id)
        .expect("lookup")
        .map(|e| e.id)
}

fn title_match(state: &AppState, kitsu_id: &str) {
    crate::commands::kitsu::title_match_put(
        state,
        ShowKey::parse(SHOW).provider,
        TITLE,
        1,
        kitsu_id,
    )
    .unwrap();
}

#[tokio::test]
async fn a_deleted_recorded_entry_gives_way_to_the_rows_live_mapping() {
    for status in [404, 410] {
        let (_tmp, _mock, state) = row_recording("999", status).await;
        crate::commands::kitsu::allmanga_kitsu_put(&state, SHOW, "1").unwrap();
        assert!(kitsu_anime_detail_fails(&state, "999").await);

        assert_eq!(found(&state, "1").as_deref(), Some(SHOW), "status {status}");
        assert_eq!(found(&state, "999"), None, "status {status}");
    }
}

#[tokio::test]
async fn a_deleted_recorded_entry_gives_way_to_the_title_match_past_a_dead_mapping() {
    // A play stamps the mapping with the id it records, so the common
    // case is a mapping naming the same deleted entry: that mapping is
    // no answer either, and the title match the home page stored for
    // the row is.
    let (_tmp, _mock, state) = row_recording("999", 404).await;
    crate::commands::kitsu::allmanga_kitsu_put(&state, SHOW, "999").unwrap();
    title_match(&state, "1");
    assert!(kitsu_anime_detail_fails(&state, "999").await);

    assert_eq!(found(&state, "1").as_deref(), Some(SHOW));
    assert_eq!(found(&state, "999"), None);
}

#[tokio::test]
async fn a_deleted_recorded_entry_with_nothing_else_names_no_show() {
    let (_tmp, _mock, state) = row_recording("999", 404).await;
    assert!(kitsu_anime_detail_fails(&state, "999").await);

    assert_eq!(found(&state, "999"), None);
    assert_eq!(found(&state, "1"), None);
}

#[tokio::test]
async fn a_transient_failure_leaves_the_recorded_id_standing() {
    for status in [500, 503, 429, 403, 400] {
        let (_tmp, _mock, state) = row_recording("999", status).await;
        crate::commands::kitsu::allmanga_kitsu_put(&state, SHOW, "1").unwrap();
        title_match(&state, "1");
        assert!(kitsu_anime_detail_fails(&state, "999").await);

        assert_eq!(
            found(&state, "999").as_deref(),
            Some(SHOW),
            "status {status}"
        );
        assert_eq!(found(&state, "1"), None, "status {status}");
    }
}

#[tokio::test]
async fn an_unreachable_kitsu_leaves_the_recorded_id_standing() {
    let tmp = tempfile::tempdir().unwrap();
    let history = tmp.path().join("history");
    write_atomic(
        &history,
        &[HistoryEntry {
            ep_no: "5".into(),
            id: SHOW.into(),
            title: TITLE.into(),
            watched_at: None,
            kitsu_id: Some("999".into()),
        }],
    )
    .unwrap();
    let state = state_at(history, "http://127.0.0.1:9");
    crate::commands::kitsu::allmanga_kitsu_put(&state, SHOW, "1").unwrap();
    assert!(kitsu_anime_detail_fails(&state, "999").await);

    assert_eq!(found(&state, "999").as_deref(), Some(SHOW));
}

#[tokio::test]
async fn a_recorded_entry_kitsu_serves_again_is_the_rows_once_more() {
    let (_tmp, mock, state) = row_recording("12", 404).await;
    crate::commands::kitsu::allmanga_kitsu_put(&state, SHOW, "1").unwrap();
    assert!(kitsu_anime_detail_fails(&state, "12").await);
    assert_eq!(found(&state, "1").as_deref(), Some(SHOW));

    mock.reset().await;
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(DETAIL_FIXTURE.to_vec()),
        )
        .mount(&mock)
        .await;
    crate::commands::kitsu::kitsu_anime_detail(&state, "12")
        .await
        .expect("served again");

    assert_eq!(found(&state, "12").as_deref(), Some(SHOW));
    assert_eq!(found(&state, "1"), None);
}

async fn kitsu_anime_detail_fails(state: &AppState, id: &str) -> bool {
    crate::commands::kitsu::kitsu_anime_detail(state, id)
        .await
        .is_err()
}

fn marked(state: &AppState, id: &str) -> bool {
    crate::commands::kitsu_gone::is_gone(state, id).expect("mark read")
}

// The mark is written by reading an id a history row recorded, so it
// is history the user can remove: a clear takes every mark, and a
// delete takes the marks of the ids the removed show was known by
// that no remaining row claims.

#[tokio::test]
async fn clearing_the_history_takes_every_gone_mark() {
    let (_tmp, _mock, state) = row_recording("999", 404).await;
    assert!(kitsu_anime_detail_fails(&state, "999").await);
    assert!(marked(&state, "999"));

    history_clear(&state).expect("clear");

    assert!(!marked(&state, "999"));
}

#[tokio::test]
async fn deleting_a_show_takes_the_gone_marks_of_its_ids() {
    let (_tmp, _mock, state) = row_recording("999", 404).await;
    assert!(kitsu_anime_detail_fails(&state, "999").await);

    assert!(history_delete(&state, SHOW).expect("delete"));

    assert!(!marked(&state, "999"));
}

#[tokio::test]
async fn deleting_a_show_keeps_a_gone_mark_another_row_claims() {
    let (_tmp, _mock, state) = row_recording("999", 404).await;
    let mut rows = crate::history::read_all(&state.history_path).unwrap();
    rows.push(HistoryEntry {
        ep_no: "2".into(),
        id: "hianime:cowboy-bebop-77".into(),
        title: TITLE.into(),
        watched_at: None,
        kitsu_id: Some("999".into()),
    });
    write_atomic(&state.history_path, &rows).unwrap();
    assert!(kitsu_anime_detail_fails(&state, "999").await);

    assert!(history_delete(&state, SHOW).expect("delete"));

    assert!(marked(&state, "999"));
}

// A detail read waits on Kitsu before it marks. A removal that lands
// while it waits took the marks of the ids the show was known by, and
// a 404 that arrives after it does not bring one back: the read began
// before the removal. A read begun after it is new, and marks as any
// does; a removal of another show leaves the read's mark alone.

const OTHER: &str = "hianime:trigun-3";

/// A history of two rows, `SHOW` recording 999 and `OTHER` recording
/// 555, with Kitsu answering `/anime/999` with a 404 only after a
/// pause long enough to remove history while the read waits.
async fn two_rows_with_a_slow_404() -> (tempfile::TempDir, MockServer, AppState) {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/999"))
        .respond_with(ResponseTemplate::new(404).set_delay(std::time::Duration::from_millis(400)))
        .mount(&mock)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let history = tmp.path().join("history");
    let row = |id: &str, kitsu_id: &str| HistoryEntry {
        ep_no: "5".into(),
        id: id.into(),
        title: TITLE.into(),
        watched_at: None,
        kitsu_id: Some(kitsu_id.into()),
    };
    write_atomic(&history, &[row(SHOW, "999"), row(OTHER, "555")]).unwrap();
    let state = state_at(history, &mock.uri());
    (tmp, mock, state)
}

/// Read `/anime/999`, running `removal` once Kitsu has the request and
/// before it answers.
async fn read_999_while(state: &AppState, mock: &MockServer, removal: impl FnOnce(&AppState)) {
    let read = kitsu_anime_detail_fails(state, "999");
    let remove = async {
        while mock
            .received_requests()
            .await
            .unwrap_or_default()
            .is_empty()
        {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        removal(state);
    };
    let (failed, ()) = tokio::join!(read, remove);
    assert!(failed);
}

#[tokio::test]
async fn a_404_that_arrives_after_the_show_was_deleted_marks_nothing() {
    let (_tmp, mock, state) = two_rows_with_a_slow_404().await;

    read_999_while(&state, &mock, |state| {
        assert!(history_delete(state, SHOW).expect("delete"));
    })
    .await;

    assert!(!marked(&state, "999"));
}

#[tokio::test]
async fn a_404_that_arrives_after_the_history_was_cleared_marks_nothing() {
    let (_tmp, mock, state) = two_rows_with_a_slow_404().await;

    read_999_while(&state, &mock, |state| history_clear(state).expect("clear")).await;

    assert!(!marked(&state, "999"));
}

#[tokio::test]
async fn a_404_that_arrives_after_another_show_was_deleted_still_marks() {
    let (_tmp, mock, state) = two_rows_with_a_slow_404().await;

    read_999_while(&state, &mock, |state| {
        assert!(history_delete(state, OTHER).expect("delete"));
    })
    .await;

    assert!(marked(&state, "999"));
}

#[tokio::test]
async fn a_read_begun_after_the_removal_marks_as_any_does() {
    let (_tmp, _mock, state) = two_rows_with_a_slow_404().await;
    assert!(history_delete(&state, SHOW).expect("delete"));

    assert!(kitsu_anime_detail_fails(&state, "999").await);

    assert!(marked(&state, "999"));
}

// A read is in flight from its first step, the cache read, not from
// the moment it reaches Kitsu: a removal that lands between the two
// took the marks of the show's ids, and the 404 that follows does not
// bring one back.

#[tokio::test]
async fn a_404_after_a_deletion_between_the_cache_read_and_kitsu_marks_nothing() {
    let (_tmp, _mock, state) = two_rows_with_a_slow_404().await;

    let read = crate::commands::kitsu::anime_detail_past_cache(&state, "999", |state| {
        assert!(history_delete(state, SHOW).expect("delete"));
    })
    .await;

    assert!(read.is_err());
    assert!(!marked(&state, "999"));
}
