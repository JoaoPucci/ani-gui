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
