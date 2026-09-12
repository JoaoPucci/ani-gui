//! Which history row a Kitsu entry resumes from when more than one
//! maps to it — two providers, two ids, one show.

use super::*;
use crate::app::AppState;
use crate::history::{write_atomic, HistoryEntry};
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use std::path::PathBuf;
use std::sync::Arc;

fn make_state(history_path: PathBuf) -> AppState {
    state_with(
        history_path,
        crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
    )
}

fn state_with(history_path: PathBuf, kitsu: crate::meta::kitsu::KitsuClient) -> AppState {
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
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu,
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// Two rows for one show: the primary's from an earlier watch, the
/// fallback's from a later one.
fn two_rows_for_one_show(s: &AppState, path: &std::path::Path) {
    write_atomic(
        path,
        &[
            HistoryEntry {
                ep_no: "3".into(),
                id: "the-show-77".into(),
                title: "The Show".into(),
            },
            HistoryEntry {
                ep_no: "7".into(),
                id: "hianime:the-show-9".into(),
                title: "The Show".into(),
            },
        ],
    )
    .unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(s, "the-show-77", "K1").unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(s, "hianime:the-show-9", "K1").unwrap();
}

#[test]
fn the_row_watched_last_wins() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    two_rows_for_one_show(&s, &path);
    crate::commands::kitsu::watched_at_put(&s, "the-show-77", 1_000).unwrap();
    crate::commands::kitsu::watched_at_put(&s, "hianime:the-show-9", 2_000).unwrap();
    let hit = history_by_kitsu(&s, "K1").unwrap().expect("match");
    assert_eq!(hit.id, "hianime:the-show-9");
    assert_eq!(hit.ep_no, "7");
}

#[test]
fn a_stamped_row_beats_an_unstamped_one() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    two_rows_for_one_show(&s, &path);
    crate::commands::kitsu::watched_at_put(&s, "hianime:the-show-9", 2_000).unwrap();
    let hit = history_by_kitsu(&s, "K1").unwrap().expect("match");
    assert_eq!(hit.id, "hianime:the-show-9");
}

#[test]
fn with_no_stamps_file_order_stands() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    two_rows_for_one_show(&s, &path);
    let hit = history_by_kitsu(&s, "K1").unwrap().expect("match");
    assert_eq!(hit.id, "the-show-77");
    assert_eq!(hit.ep_no, "3");
}

/// A row the guard refuses to re-map can still carry a mapping from
/// before the guard existed. Recording its watch stamps it newest,
/// and under that stale mapping it would outrank the row that maps
/// to the entry correctly; the refusal drops the stale mapping
/// instead, and the correctly mapped row is the one to resume.
#[tokio::test]
async fn a_stale_mapping_the_guard_refuses_does_not_outrank_a_correct_row() {
    use wiremock::matchers::{method, path as url_path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    const DETAIL: &[u8] =
        include_bytes!("../../../tests/fixtures/kitsu/anime_one_piece_detail.json");
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(url_path("/anime/12"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(DETAIL.to_vec()),
        )
        .mount(&mock)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = state_with(
        path.clone(),
        crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), mock.uri()),
    );
    write_atomic(
        &path,
        &[
            HistoryEntry {
                ep_no: "7".into(),
                id: "hianime:one-piece-100".into(),
                title: "One Piece".into(),
            },
            HistoryEntry {
                ep_no: "3".into(),
                id: "one-piece-69".into(),
                title: "One Piece Part 2".into(),
            },
        ],
    )
    .unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(&s, "hianime:one-piece-100", "12").unwrap();
    crate::commands::kitsu::watched_at_put(&s, "hianime:one-piece-100", 1_000).unwrap();
    // The Part 2 row's mapping to the cour-1 entry predates the guard.
    crate::commands::kitsu::allmanga_kitsu_put(&s, "one-piece-69", "12").unwrap();
    let watch = crate::commands::play_native_record::Watch {
        show_id: "one-piece-69".into(),
        title: "One Piece Part 2".into(),
        ep_no: "4".into(),
    };
    crate::commands::play_native_record::record_watch(&s, &watch, Some("12")).await;
    let hit = history_by_kitsu(&s, "12").unwrap().expect("match");
    assert_eq!(hit.id, "hianime:one-piece-100");
    assert_eq!(hit.ep_no, "7");
}
