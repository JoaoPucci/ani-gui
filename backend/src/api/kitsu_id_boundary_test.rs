//! Every route that takes a Kitsu id from the renderer decides at its
//! boundary whether the value is one. A required id that is not one
//! answers 400 `invalid_kitsu_id` before anything is read, written or
//! requested; an optional one reads as no id at all.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;
use wiremock::MockServer;

use crate::app::AppState;
use crate::commands::availability::{
    AvailabilityArgs, AvailabilityBatchArgs, AvailabilityWarmArgs,
};
use crate::commands::download::DownloadArgs;
use crate::commands::play::PlayArgs;

/// App state whose Kitsu client points at `kitsu`, so a test can see
/// whether a route asked Kitsu for anything.
fn state(td: &TempDir, kitsu: &str) -> Arc<AppState> {
    Arc::new(AppState {
        anidb_base: None,
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: td.path().join("history"),
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        hianime_base: None,
        hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        provider_order: vec![crate::scraper::provider::ProviderId::Anidb],
        image_cache_dir: td.path().join("images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu),
        config_path: td.path().join("config.toml"),
        state_dir: td.path().join("state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

async fn send(state: Arc<AppState>, method: &str, uri: &str, body: &str) -> (StatusCode, String) {
    let r = super::build_api_router(state)
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .header("authorization", "Bearer t")
                .body(Body::from(body.to_owned()))
                .expect("req"),
        )
        .await
        .expect("oneshot");
    let status = r.status();
    let bytes = r.into_body().collect().await.expect("body").to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// The id forms the review found reaching keys and URLs, percent-
/// encoded where they sit in a path segment.
const NOT_IDS: [&str; 4] = ["..%2F49877", "49877%2Fx", "12:21", "kid-1"];

/// A stand-in Kitsu that answers nothing and counts every connection
/// made to it. Spawned on the test's own runtime: a pooled mock
/// server per call deadlocked on drop when a test ran dozens of them.
async fn counting_kitsu() -> (String, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    tokio::spawn(async move {
        while let Ok((conn, _)) = listener.accept().await {
            seen.fetch_add(1, Ordering::SeqCst);
            drop(conn);
        }
    });
    (base, count)
}

/// Each call runs against fresh state, so nothing one refusal could
/// have written is visible to the next; the stand-in Kitsu is shared
/// so the test can assert at the end that nothing reached it.
async fn assert_refused(kitsu: &str, method: &str, uri: &str, body: &str) {
    let td = TempDir::new().expect("tempdir");
    let (status, text) = send(state(&td, kitsu), method, uri, body).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "{method} {uri} {body}: {text}"
    );
    let v: serde_json::Value = serde_json::from_str(&text).expect("json error body");
    assert_eq!(
        v["kind"], "invalid_kitsu_id",
        "{method} {uri} {body}: {text}"
    );
}

#[tokio::test]
async fn the_id_routes_refuse_a_value_that_is_not_an_id() {
    let (kitsu, asked) = counting_kitsu().await;
    let kitsu = kitsu.as_str();
    for id in NOT_IDS {
        assert_refused(kitsu, "GET", &format!("/api/kitsu/anime/{id}"), "").await;
        assert_refused(
            kitsu,
            "GET",
            &format!("/api/kitsu/episodes/{id}?page=1"),
            "",
        )
        .await;
        assert_refused(
            kitsu,
            "GET",
            &format!("/api/kitsu/episodes/{id}?page=1&refresh=true"),
            "",
        )
        .await;
        assert_refused(kitsu, "GET", &format!("/api/kitsu/airing/{id}"), "").await;
        assert_refused(
            kitsu,
            "GET",
            &format!("/api/kitsu/airing/{id}?refresh=true"),
            "",
        )
        .await;
        assert_refused(
            kitsu,
            "GET",
            &format!("/api/aniskip/{id}/1?episode_length=1400"),
            "",
        )
        .await;
        assert_refused(kitsu, "GET", &format!("/api/history/by-kitsu/{id}"), "").await;
        assert_refused(
            kitsu,
            "GET",
            &format!("/api/account/entry/anilist?kitsu_id={id}"),
            "",
        )
        .await;
        assert_refused(
            kitsu,
            "DELETE",
            &format!("/api/account/entry/anilist?kitsu_id={id}"),
            "",
        )
        .await;
    }
    for method in ["GET", "DELETE"] {
        assert_refused(kitsu, method, "/api/account/entry/anilist?kitsu_id=", "").await;
    }
    assert_eq!(
        asked.load(Ordering::SeqCst),
        0,
        "a refused route asked Kitsu"
    );
}

#[tokio::test]
async fn the_id_bodies_refuse_a_value_that_is_not_an_id() {
    let (kitsu, asked) = counting_kitsu().await;
    let kitsu = kitsu.as_str();
    for id in ["../49877", "49877/x", "12:21", "kid-1", ""] {
        assert_refused(
            kitsu,
            "POST",
            "/api/account/update/anilist",
            &format!(r#"{{"kitsu_id":"{id}","progress":3}}"#),
        )
        .await;
        assert_refused(
            kitsu,
            "POST",
            "/api/account/set/anilist",
            &format!(r#"{{"kitsu_id":"{id}","status":"watching"}}"#),
        )
        .await;
        assert_refused(
            kitsu,
            "PUT",
            "/api/title-match",
            &format!(r#"{{"title":"Naruto","cour":1,"kitsu_id":"{id}"}}"#),
        )
        .await;
    }
    assert_eq!(
        asked.load(Ordering::SeqCst),
        0,
        "a refused route asked Kitsu"
    );
}

#[tokio::test]
async fn a_refused_title_match_stores_nothing() {
    let td = TempDir::new().expect("tempdir");
    let s = state(&td, "http://127.0.0.1:1");
    let body = r#"{"title":"Naruto","cour":1,"kitsu_id":"../49877"}"#;
    let (status, _) = send(s.clone(), "PUT", "/api/title-match", body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, text) = send(s, "GET", "/api/title-match?title=Naruto&cour=1", "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(text, "null");
}

#[tokio::test]
async fn a_padded_id_asks_kitsu_for_its_digits() {
    let td = TempDir::new().expect("tempdir");
    let kitsu = MockServer::start().await;
    let s = state(&td, &kitsu.uri());
    let _ = send(s, "GET", "/api/kitsu/anime/%2049877%20", "").await;
    let asked = kitsu.received_requests().await.unwrap_or_default();
    assert!(!asked.is_empty(), "the detail read asked Kitsu nothing");
    for r in asked {
        assert_eq!(r.url.path(), "/anime/49877", "asked for {}", r.url);
    }
}

fn play_body(id: &str) -> String {
    format!(r#"{{"title":"Naruto","episode":"1","mode":"sub","kitsu_id":"{id}"}}"#)
}

#[test]
fn play_and_download_args_read_a_non_id_as_no_id() {
    for id in ["../49877", "49877/x", "12:21", "kid-1", ""] {
        let play: PlayArgs = serde_json::from_str(&play_body(id)).expect("play json");
        assert_eq!(play.kitsu_id, None, "play json {id:?}");
        let dl: DownloadArgs = serde_json::from_str(&play_body(id)).expect("download json");
        assert_eq!(dl.kitsu_id, None, "download json {id:?}");
        let q = format!(
            "title=Naruto&episode=1&mode=sub&kitsu_id={}",
            id.replace('/', "%2F").replace(':', "%3A")
        );
        let play: PlayArgs = serde_urlencoded::from_str(&q).expect("play query");
        assert_eq!(play.kitsu_id, None, "play query {id:?}");
        let dl: DownloadArgs = serde_urlencoded::from_str(&q).expect("download query");
        assert_eq!(dl.kitsu_id, None, "download query {id:?}");
    }
    let play: PlayArgs = serde_json::from_str(&play_body(" 49877 ")).expect("play json");
    assert_eq!(play.kitsu_id.as_deref(), Some("49877"));
}

#[test]
fn availability_args_read_a_non_id_as_no_id() {
    let body = |id: &str| format!(r#"{{"title":"Naruto","mode":"sub","kitsu_id":"{id}"}}"#);
    for id in ["../49877", "49877/x", "12:21", "kid-1"] {
        let one: AvailabilityArgs = serde_json::from_str(&body(id)).expect("check json");
        assert_eq!(one.kitsu_id, None, "check {id:?}");
        let warm: AvailabilityWarmArgs =
            serde_json::from_str(&format!(r#"{{"items":[{}]}}"#, body(id))).expect("warm json");
        assert_eq!(warm.items[0].kitsu_id, None, "warm {id:?}");
    }
    let batch: AvailabilityBatchArgs =
        serde_json::from_str(r#"{"kitsu_ids":["1","../2"," 3 ","4/x","5:6"],"mode":"sub"}"#)
            .expect("batch json");
    assert_eq!(batch.kitsu_ids, vec!["1", "3"]);
}
