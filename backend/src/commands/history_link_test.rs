//! Linking history rows to a show the user names, and undoing it.

use super::*;
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::history::{read_all, write_atomic, HistoryEntry};
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use std::path::PathBuf;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DETAIL_FIXTURE: &[u8] =
    include_bytes!("../../../tests/fixtures/kitsu/anime_one_piece_detail.json");
/// The id the fixture is: the show the user names.
const LINKED: &str = "12";

fn state_at(history_path: PathBuf, kitsu_base: &str) -> AppState {
    AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 0),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path,
        providers: crate::provider_set::ProviderSet::for_tests(),
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

fn row(ep: &str, id: &str, title: &str, at: Option<i64>, kitsu: Option<&str>) -> HistoryEntry {
    HistoryEntry {
        ep_no: ep.into(),
        id: id.into(),
        title: title.into(),
        watched_at: at,
        kitsu_id: kitsu.map(str::to_string),
        own_number: false,
    }
}

/// The card's row, a hidden bare row the card stands for, and another
/// show's row.
fn rows() -> Vec<HistoryEntry> {
    vec![
        row(
            "1",
            "hianime:black-clover-949",
            "Black Clover",
            Some(1_791_253_563_411),
            None,
        ),
        row("3", "black-clover-77", "Black Clover", None, None),
        row(
            "175",
            "hianime:gintama-1237",
            "Gintama",
            Some(1_793_504_410_489),
            Some("818"),
        ),
    ]
}

/// A history of [`rows`], with Kitsu answering `/anime/12` with
/// `status` (the fixture on 200).
async fn setup(status: u16) -> (tempfile::TempDir, MockServer, AppState) {
    let mock = MockServer::start().await;
    let response = if status == 200 {
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/vnd.api+json")
            .set_body_bytes(DETAIL_FIXTURE.to_vec())
    } else {
        ResponseTemplate::new(status)
    };
    Mock::given(method("GET"))
        .and(path(format!("/anime/{LINKED}")))
        .respond_with(response)
        .mount(&mock)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let history = tmp.path().join("history");
    write_atomic(&history, &rows()).unwrap();
    let state = state_at(history, &mock.uri());
    (tmp, mock, state)
}

fn args(rows: &[&str], kitsu_id: &str) -> LinkArgs {
    LinkArgs {
        rows: rows.iter().map(|r| (*r).to_string()).collect(),
        kitsu_id: kitsu_id.into(),
    }
}

fn file(state: &AppState) -> String {
    std::fs::read_to_string(&state.history_path).unwrap()
}

#[tokio::test]
async fn a_link_writes_the_column_of_every_named_row_and_nothing_else() {
    let (_tmp, _mock, state) = setup(200).await;
    let before = rows();

    let outcome = link(
        &state,
        &args(&["hianime:black-clover-949", "black-clover-77"], LINKED),
    )
    .await
    .expect("link");

    let after = read_all(&state.history_path).unwrap();
    let mut expected = before.clone();
    expected[0].kitsu_id = Some(LINKED.into());
    expected[1].kitsu_id = Some(LINKED.into());
    assert_eq!(after, expected, "episode, moment, title and order stay");
    assert_eq!(
        outcome.linked,
        [
            LinkedRow {
                row: "hianime:black-clover-949".into(),
                previous: None,
                ep_no: "1".into(),
                watched_at: Some(1_791_253_563_411),
            },
            LinkedRow {
                row: "black-clover-77".into(),
                previous: None,
                ep_no: "3".into(),
                watched_at: None,
            },
        ]
    );
    assert_eq!(outcome.kitsu_id, LINKED);
}

#[tokio::test]
async fn a_link_moves_no_stamp_and_writes_no_played_mark() {
    let (_tmp, _mock, state) = setup(200).await;
    let stamp = "watched-at:v1:hianime:black-clover-949";
    meta_cache_put(&state.cache_pool, stamp, "1791253563411", 86_400).unwrap();

    link(&state, &args(&["hianime:black-clover-949"], LINKED))
        .await
        .expect("link");

    assert_eq!(
        meta_cache_get(&state.cache_pool, stamp).unwrap().as_deref(),
        Some("1791253563411")
    );
    let mark = crate::commands::kitsu::allmanga_kitsu_played_key("hianime:black-clover-949");
    assert_eq!(meta_cache_get(&state.cache_pool, &mark).unwrap(), None);
}

#[tokio::test]
async fn a_mapping_naming_another_show_goes_and_one_naming_the_linked_show_stays() {
    let (_tmp, _mock, state) = setup(200).await;
    crate::commands::kitsu::allmanga_kitsu_put(&state, "hianime:black-clover-949", "12625")
        .unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(&state, "black-clover-77", LINKED).unwrap();

    link(
        &state,
        &args(&["hianime:black-clover-949", "black-clover-77"], LINKED),
    )
    .await
    .expect("link");

    let get = |id| crate::commands::kitsu::allmanga_kitsu_get(&state, id).unwrap();
    assert_eq!(get("hianime:black-clover-949"), None);
    assert_eq!(get("black-clover-77").as_deref(), Some(LINKED));
}

#[tokio::test]
async fn an_id_kitsu_answers_gone_is_refused_and_nothing_is_written() {
    for status in [404, 410] {
        let (_tmp, _mock, state) = setup(status).await;
        let before = file(&state);

        let err = link(&state, &args(&["hianime:black-clover-949"], LINKED))
            .await
            .expect_err("a gone id is no link");

        assert!(
            matches!(err, crate::error::AniError::Upstream { status: s } if s == status),
            "{err:?}"
        );
        assert_eq!(file(&state), before);
    }
}

#[tokio::test]
async fn a_value_that_is_not_an_id_is_refused() {
    let (_tmp, _mock, state) = setup(200).await;
    let before = file(&state);
    let err = link(&state, &args(&["hianime:black-clover-949"], "abc"))
        .await
        .expect_err("no id");
    assert!(matches!(err, crate::error::AniError::InvalidKitsuId));
    assert_eq!(file(&state), before);
}

#[tokio::test]
async fn a_row_the_history_does_not_hold_is_not_written() {
    let (_tmp, _mock, state) = setup(200).await;
    let outcome = link(&state, &args(&["hianime:never-played-1"], LINKED))
        .await
        .expect("link");
    assert!(outcome.linked.is_empty());
    assert_eq!(read_all(&state.history_path).unwrap(), rows());
}

#[tokio::test]
async fn a_row_removed_while_kitsu_was_asked_is_not_linked_when_played_again() {
    let (_tmp, _mock, state) = setup(200).await;
    let begun = crate::history::guard::epoch(&state.history_path);
    // Removed, then written again by a later play, while the link
    // waited on Kitsu: the row is that play's now.
    crate::commands::history::history_delete(&state, "hianime:black-clover-949").unwrap();
    crate::history::upsert_and_write(
        &state.history_path,
        row(
            "2",
            "hianime:black-clover-949",
            "Black Clover",
            Some(1_793_000_000_000),
            None,
        ),
    )
    .unwrap();

    let outcome = crate::history::guard::hold(&state.history_path, |held| {
        link_held(
            &state,
            held,
            &["hianime:black-clover-949".to_string()],
            LINKED,
            begun,
        )
    })
    .expect("link");

    assert!(outcome.linked.is_empty());
    let after = read_all(&state.history_path).unwrap();
    let played = after
        .iter()
        .find(|e| e.id == "hianime:black-clover-949")
        .unwrap();
    assert_eq!(played.kitsu_id, None);
}

#[tokio::test]
async fn an_undo_gives_rows_back_their_id_unless_written_since() {
    let (_tmp, _mock, state) = setup(200).await;
    let outcome = link(
        &state,
        &args(&["hianime:black-clover-949", "black-clover-77"], LINKED),
    )
    .await
    .expect("link");
    // A watch of the card's row lands after the link.
    crate::history::upsert_and_write(
        &state.history_path,
        row(
            "2",
            "hianime:black-clover-949",
            "Black Clover",
            Some(1_793_000_000_000),
            Some(LINKED),
        ),
    )
    .unwrap();

    let undone = unlink(
        &state,
        &UnlinkArgs {
            kitsu_id: LINKED.into(),
            rows: outcome.linked,
        },
    )
    .expect("unlink");

    assert_eq!(undone.restored, ["black-clover-77"]);
    assert_eq!(undone.kept, ["hianime:black-clover-949"]);
    let after = read_all(&state.history_path).unwrap();
    assert_eq!(after[0].kitsu_id.as_deref(), Some(LINKED));
    assert_eq!(after[1], rows()[1]);
}

#[tokio::test]
async fn a_relink_undone_gives_the_row_back_the_id_it_had() {
    let (_tmp, _mock, state) = setup(200).await;
    let outcome = link(&state, &args(&["hianime:gintama-1237"], LINKED))
        .await
        .expect("link");
    assert_eq!(outcome.linked[0].previous.as_deref(), Some("818"));

    let undone = unlink(
        &state,
        &UnlinkArgs {
            kitsu_id: LINKED.into(),
            rows: outcome.linked,
        },
    )
    .expect("unlink");

    assert_eq!(undone.restored, ["hianime:gintama-1237"]);
    assert_eq!(read_all(&state.history_path).unwrap(), rows());
}

#[tokio::test]
async fn the_routes_link_and_undo() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;

    let (_tmp, _mock, state) = setup(200).await;
    let history = state.history_path.clone();
    let router = crate::api::build_api_router(Arc::new(state));
    let post = |uri: &str, body: serde_json::Value| {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };

    let response = router
        .clone()
        .oneshot(post(
            "/api/history/link",
            serde_json::json!({"rows": ["black-clover-77"], "kitsu_id": LINKED}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["linked"][0]["row"], "black-clover-77");
    assert_eq!(
        read_all(&history).unwrap()[1].kitsu_id.as_deref(),
        Some(LINKED)
    );

    let response = router
        .oneshot(post(
            "/api/history/unlink",
            serde_json::json!({"kitsu_id": LINKED, "rows": body["linked"]}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(read_all(&history).unwrap(), rows());
}
