//! Linking history rows to a show the user names.

use super::*;
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::commands::link_verdict::RowMatch;
use crate::history::{read_all, write_atomic, HistoryEntry};
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use std::path::PathBuf;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DETAIL_FIXTURE: &[u8] =
    include_bytes!("../../../tests/fixtures/kitsu/anime_one_piece_detail.json");
/// The id the fixture is: the show the user names.
const LINKED: &str = "12";

/// A state whose files sit beside `history_path`, in the test's own
/// directory.
fn state_at(history_path: PathBuf, kitsu_base: &str) -> AppState {
    let dir = history_path
        .parent()
        .expect("a history path inside the test's directory")
        .to_path_buf();
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
        image_cache_dir: dir.join("images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_base),
        config_path: dir.join("config.toml"),
        state_dir: dir.join("state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
        flags: crate::flags::FlagStore::fixed(crate::flags::DECLARED),
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
        card: rows[0].to_string(),
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
async fn the_route_links() {
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
            serde_json::json!({"card": "black-clover-77", "rows": ["black-clover-77"], "kitsu_id": LINKED}),
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
}

/// The JoJo card: its own row, the 2000 OVA's listing, guessed as the
/// 2012 series, and the row its guessed play wrote for the 2012
/// series' listing — another listing, guessed as the same show.
fn jojo_rows() -> Vec<HistoryEntry> {
    vec![
        row(
            "2",
            "hianime:jojos-bizarre-adventure-2012-708",
            "JoJo's Bizarre Adventure",
            Some(1_793_600_000_000),
            None,
        ),
        row(
            "1",
            "hianime:jojos-bizarre-adventure-2000-5182",
            "JoJo's Bizarre Adventure",
            Some(1_791_000_000_000),
            None,
        ),
    ]
}

const JOJO_CARD: &str = "hianime:jojos-bizarre-adventure-2000-5182";
const JOJO_PLAYED: &str = "hianime:jojos-bizarre-adventure-2012-708";

#[tokio::test]
async fn a_link_leaves_out_the_row_another_listing_wrote() {
    let (_tmp, _mock, state) = setup(200).await;
    write_atomic(&state.history_path, &jojo_rows()).unwrap();

    let outcome = link(
        &state,
        &LinkArgs {
            card: JOJO_CARD.into(),
            rows: vec![JOJO_CARD.into(), JOJO_PLAYED.into()],
            kitsu_id: LINKED.into(),
        },
    )
    .await
    .expect("link");

    let mut expected = jojo_rows();
    expected[1].kitsu_id = Some(LINKED.into());
    assert_eq!(
        read_all(&state.history_path).unwrap(),
        expected,
        "the 2012 listing's row stays the 2012 series' history"
    );
    let linked: Vec<&str> = outcome.linked.iter().map(|l| l.row.as_str()).collect();
    assert_eq!(linked, [JOJO_CARD]);
}

#[tokio::test]
async fn a_link_keeps_a_bare_row_of_the_cards_own_listing() {
    let (_tmp, _mock, state) = setup(200).await;

    let outcome = link(
        &state,
        &LinkArgs {
            card: "hianime:black-clover-949".into(),
            rows: vec![
                "hianime:black-clover-949".into(),
                "black-clover-77".into(),
                "hianime:gintama-1237".into(),
            ],
            kitsu_id: LINKED.into(),
        },
    )
    .await
    .expect("link");

    let linked: Vec<&str> = outcome.linked.iter().map(|l| l.row.as_str()).collect();
    assert_eq!(linked, ["hianime:black-clover-949", "black-clover-77"]);
}

#[test]
fn the_group_a_link_would_write_leaves_out_another_listing() {
    let tmp = tempfile::tempdir().unwrap();
    let history = tmp.path().join("history");
    write_atomic(&history, &jojo_rows()).unwrap();
    let state = state_at(history, "http://127.0.0.1:9");

    let guess = |id: &str| RowMatch {
        id: id.into(),
        show: Some("7158".into()),
        trusted: false,
    };
    let group = link_group(
        &state,
        &LinkGroupArgs {
            card: JOJO_CARD.into(),
            show: "7158".into(),
            saved: None,
            rows: vec![guess(JOJO_PLAYED), guess("hianime:gone-1")],
        },
    )
    .expect("group");

    assert_eq!(group, [JOJO_CARD]);
}

#[test]
fn the_group_is_the_card_and_the_rows_guessed_as_its_show_on_its_listing() {
    let tmp = tempfile::tempdir().unwrap();
    let history = tmp.path().join("history");
    write_atomic(&history, &rows()).unwrap();
    let state = state_at(history, "http://127.0.0.1:9");
    let matched = |id: &str, show: &str, trusted: bool| RowMatch {
        id: id.into(),
        show: Some(show.into()),
        trusted,
    };

    let group = link_group(
        &state,
        &LinkGroupArgs {
            card: "hianime:black-clover-949".into(),
            show: "12625".into(),
            saved: None,
            rows: vec![
                matched("black-clover-77", "12625", false),
                matched("hianime:gintama-1237", "12625", true),
            ],
        },
    )
    .expect("group");

    assert_eq!(group, ["hianime:black-clover-949", "black-clover-77"]);
}

#[tokio::test]
async fn the_group_route_answers_the_rows_a_link_would_write() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;

    let (_tmp, _mock, state) = setup(200).await;
    let router = crate::api::build_api_router(Arc::new(state));
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/history/link-group")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "card": "black-clover-77",
                        "show": "12625",
                        "rows": [
                            { "id": "hianime:black-clover-949", "show": "12625", "trusted": false },
                            { "id": "hianime:gintama-1237", "show": "818", "trusted": false }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        body,
        serde_json::json!(["black-clover-77", "hianime:black-clover-949"])
    );
}

#[tokio::test]
async fn a_link_notes_the_rows_it_linked() {
    let (_tmp, _mock, state) = setup(200).await;
    let note = |row: &str| crate::commands::linked_note::linked(&state, row);
    link(
        &state,
        &args(&["hianime:black-clover-949", "black-clover-77"], LINKED),
    )
    .await
    .expect("link");
    assert_eq!(note("hianime:black-clover-949").as_deref(), Some(LINKED));
    assert_eq!(note("black-clover-77").as_deref(), Some(LINKED));
    assert_eq!(note("hianime:gintama-1237"), None);
}

#[tokio::test]
async fn forgetting_a_row_drops_its_link_note() {
    let (_tmp, _mock, state) = setup(200).await;
    link(&state, &args(&["hianime:black-clover-949"], LINKED))
        .await
        .expect("link");
    crate::commands::history_forget::forget_show(
        &state,
        "hianime:black-clover-949",
        "Black Clover",
        Some(LINKED),
        &[],
        &std::collections::HashSet::new(),
    )
    .expect("forget");
    assert_eq!(
        crate::commands::linked_note::linked(&state, "hianime:black-clover-949"),
        None
    );
}

#[tokio::test]
async fn clearing_the_history_drops_every_link_note() {
    let (_tmp, _mock, state) = setup(200).await;
    link(
        &state,
        &args(&["hianime:black-clover-949", "black-clover-77"], LINKED),
    )
    .await
    .expect("link");
    crate::commands::history_forget::forget_all(&state).expect("clear");
    for row in ["hianime:black-clover-949", "black-clover-77"] {
        assert_eq!(
            crate::commands::linked_note::linked(&state, row),
            None,
            "{row}"
        );
    }
}

/// The history route says which rows record a show the user named, so
/// every surface can offer to change it again, across restarts: the
/// link's note stands while the row records the id it noted.
#[tokio::test]
async fn the_history_route_marks_the_rows_the_user_named_a_show_for() {
    use axum::body::Body;
    use axum::http::Request;
    use std::sync::Arc;
    use tower::ServiceExt;

    let (_tmp, _mock, state) = setup(200).await;
    link(&state, &args(&["black-clover-77"], LINKED))
        .await
        .expect("link");
    let history = state.history_path.clone();
    let router = crate::api::build_api_router(Arc::new(state));
    let named = |router: axum::Router| async move {
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/history")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        rows.iter()
            .filter(|r| r["named"] == serde_json::Value::Bool(true))
            .map(|r| r["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    // The linked row is named; a row whose id a play recorded is not.
    assert_eq!(named(router.clone()).await, vec!["black-clover-77"]);

    // A play from another show's page records another id on the row:
    // the note no longer speaks for it.
    crate::history::upsert_and_write(
        &history,
        row(
            "4",
            "black-clover-77",
            "Black Clover",
            Some(1_793_000_000_000),
            Some("999"),
        ),
    )
    .unwrap();
    assert!(named(router).await.is_empty());
}

#[tokio::test]
async fn a_link_that_waited_on_kitsu_leaves_a_newer_link_standing() {
    // The first correction waits for its Kitsu read; the viewer corrects
    // the same card again on another surface, which saves first. The
    // older request lands last and must not overwrite the newer one.
    let mock = MockServer::start().await;
    let detail = || {
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/vnd.api+json")
            .set_body_bytes(DETAIL_FIXTURE.to_vec())
    };
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(detail().set_delay(std::time::Duration::from_millis(400)))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/21"))
        .respond_with(detail())
        .mount(&mock)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let history = tmp.path().join("history");
    write_atomic(&history, &rows()).unwrap();
    let state = state_at(history, &mock.uri());
    let card = ["hianime:black-clover-949"];

    let first = args(&card, "12");
    let second = args(&card, "21");
    let (older, newer) = tokio::join!(link(&state, &first), async {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        link(&state, &second).await
    });

    assert_eq!(newer.expect("newer").linked.len(), 1);
    let older = older.expect("older");
    assert!(older.linked.is_empty(), "the older link writes nothing");
    assert!(older.overtaken, "and says a newer answer stands");
    let row = read_all(&state.history_path)
        .unwrap()
        .into_iter()
        .find(|r| r.id == card[0])
        .unwrap();
    assert_eq!(row.kitsu_id.as_deref(), Some("21"));
    assert_eq!(
        crate::commands::linked_note::linked(&state, card[0]).as_deref(),
        Some("21")
    );
}

#[tokio::test]
async fn the_link_asked_last_stands_whichever_saves_first() {
    // The first correction's read answers first; the second, asked
    // later, waits longer. The second is the viewer's latest answer.
    let mock = MockServer::start().await;
    let detail = |ms: u64| {
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/vnd.api+json")
            .set_body_bytes(DETAIL_FIXTURE.to_vec())
            .set_delay(std::time::Duration::from_millis(ms))
    };
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(detail(100))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/21"))
        .respond_with(detail(500))
        .mount(&mock)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let history = tmp.path().join("history");
    write_atomic(&history, &rows()).unwrap();
    let state = state_at(history, &mock.uri());
    let card = ["hianime:black-clover-949"];
    let first = args(&card, "12");
    let second = args(&card, "21");

    let (older, newer) = tokio::join!(link(&state, &first), async {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        link(&state, &second).await
    });

    assert!(!older.expect("older").overtaken);
    let newer = newer.expect("newer");
    assert!(!newer.overtaken);
    assert_eq!(newer.linked.len(), 1);
    let row = read_all(&state.history_path)
        .unwrap()
        .into_iter()
        .find(|r| r.id == card[0])
        .unwrap();
    assert_eq!(row.kitsu_id.as_deref(), Some("21"));
}

#[tokio::test]
async fn an_overtaken_link_writes_none_of_its_rows() {
    // The older correction carries the card and its bare row; the newer
    // one, which saves first, carries the card alone. The older one must
    // not split the card's rows across two shows.
    let mock = MockServer::start().await;
    let detail = |ms: u64| {
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/vnd.api+json")
            .set_body_bytes(DETAIL_FIXTURE.to_vec())
            .set_delay(std::time::Duration::from_millis(ms))
    };
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(detail(400))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/21"))
        .respond_with(detail(0))
        .mount(&mock)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let history = tmp.path().join("history");
    write_atomic(&history, &rows()).unwrap();
    let state = state_at(history, &mock.uri());
    // The bare row's stored mapping names another show; a link to 12
    // would drop it.
    crate::commands::kitsu::allmanga_kitsu_put(&state, "black-clover-77", "99").expect("seed");
    let first = args(&["hianime:black-clover-949", "black-clover-77"], "12");
    let second = args(&["hianime:black-clover-949"], "21");

    let (older, newer) = tokio::join!(link(&state, &first), async {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        link(&state, &second).await
    });

    newer.expect("newer");
    let older = older.expect("older");
    assert!(older.overtaken);
    assert!(older.linked.is_empty(), "the older link writes no row");
    let bare = read_all(&state.history_path)
        .unwrap()
        .into_iter()
        .find(|r| r.id == "black-clover-77")
        .unwrap();
    assert_ne!(bare.kitsu_id.as_deref(), Some("12"));
    assert_eq!(
        crate::commands::linked_note::linked(&state, "black-clover-77"),
        None
    );
    assert!(
        crate::commands::kitsu::allmanga_kitsu_get(&state, "black-clover-77")
            .expect("read")
            .is_some(),
        "its stored mapping stays"
    );
}
