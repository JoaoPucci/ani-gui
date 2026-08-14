//! End-to-end integration test for the play endpoints.
//!
//! Drives `POST /api/play` through the full axum router and verifies
//! the chain
//!
//!   POST /api/play  →  native walk against the provider
//!                  →  episode → languages → embed → master URL
//!                  →  create_session wraps the upstream URL
//!                  →  return CreateSessionResponse
//!
//! works end-to-end. The unit tests in `api/mod.rs` cover the route's
//! body validation; this file covers the resolution path behind it.
//!
//! Hermetic: every hop answers from a local stub, pointed at through
//! `AppState::anidb_base`. It has to be — a live resolve inside an
//! integration test turns a throttled IP into a network 503 that
//! reads exactly like a regression in the diff under review.
//!
//! Linux-only: the proxy half of the assertion depends on a POSIX
//! temp-dir layout the Windows runner doesn't reproduce.

#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::sync::Arc;

use ani_gui::api::build_api_router;
use ani_gui::app::AppState;
use ani_gui::cache;
use ani_gui::meta::kitsu::KitsuClient;
use ani_gui::proxy::{AppSecret, ProxyOrigin, SessionTable};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

/// Stand up a local anidb stub covering the whole native flow:
/// browse → episodes → languages → embed → master URL. The embed and
/// master both live on the stub's own origin so every request stays
/// on the machine. The transport is the production one — the resolved
/// system curl works fine against localhost; impersonation only
/// matters at the real cloudflare front.
async fn stub_anidb() -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    let base = server.uri();
    let browse = r#"<a href="/anime/test-show-1"><img alt="test"/></a>"#;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/browse"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(browse))
        .mount(&server)
        .await;
    let eps: Vec<String> = (1..=12)
        .map(|n| format!("{{\"id\":{},\"number\":{}}}", 1000 + n, n))
        .collect();
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/frontend/anime/1/episodes"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(format!("{{\"episodes\":[{}]}}", eps.join(","))),
        )
        .mount(&server)
        .await;
    let langs =
        format!("{{\"languages\":[{{\"code\":\"jpn\",\"embed_url\":\"{base}/embed/x\"}}]}}");
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(
            "/api/frontend/episode/1001/languages",
        ))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(langs))
        .mount(&server)
        .await;
    let embed = format!("player.setup({{ file: '{base}/op/master.m3u8' }});");
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/embed/x"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(embed))
        .mount(&server)
        .await;
    // The quality step validates the master on every path now, best
    // included — the stub serves it like the CDN would.
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/op/master.m3u8"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("#EXTM3U\n"))
        .mount(&server)
        .await;
    server
}

/// Build an `AppState` whose provider base points at the local anidb
/// stub and whose `ani_cli_path` names a file that does not exist —
/// the native play path must never spawn the script, and a state
/// that would try dies loudly.
fn build_state(tmp: &std::path::Path, anidb_base: &str) -> AppState {
    AppState {
        anidb_base: Some(anidb_base.to_string()),
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: ani_gui::legacy_script::SweepReport::default(),
        history_path: tmp.join("hist/ani-hsts"),
        anidb_gate: Arc::new(ani_gui::scraper::gate::ScraperGate::new()),
        image_cache_dir: tmp.join("images"),
        cache_pool: cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::with_base(reqwest::Client::new(), "http://127.0.0.1:1"),
        config_path: tmp.join("config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: ani_gui::account::InternalSecret::random(),
        mal_refresh: ani_gui::meta::mal_user::MalRefreshState::new(),
        account_write_locks: ani_gui::commands::account::AccountWriteLocks::new(),
        availability_refreshes: ani_gui::commands::availability_refresh::AvailabilityRefreshes::new(
        ),
    }
}

/// The same provider, but the embed hands out an opaque playlist URL
/// — no `.m3u8` tail, the fast4speed shape — and the CDN answers no
/// HEAD at all.
async fn stub_anidb_opaque() -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    let base = server.uri();
    let browse = r#"<a href="/anime/test-show-1"><img alt="test"/></a>"#;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/browse"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(browse))
        .mount(&server)
        .await;
    let eps: Vec<String> = (1..=12)
        .map(|n| format!("{{\"id\":{},\"number\":{}}}", 1000 + n, n))
        .collect();
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/frontend/anime/1/episodes"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(format!("{{\"episodes\":[{}]}}", eps.join(","))),
        )
        .mount(&server)
        .await;
    let langs =
        format!("{{\"languages\":[{{\"code\":\"jpn\",\"embed_url\":\"{base}/embed/x\"}}]}}");
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(
            "/api/frontend/episode/1001/languages",
        ))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(langs))
        .mount(&server)
        .await;
    let embed = format!("player.setup({{ file: '{base}/op/stream/sub/1' }});");
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/embed/x"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(embed))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/op/stream/sub/1"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("#EXTM3U\n"))
        .mount(&server)
        .await;
    // No HEAD route at all: wiremock answers 404, the shape of a CDN
    // that rejects the method.
    server
}

#[tokio::test]
async fn an_opaque_validated_playlist_stays_hls() {
    // The resolve fetched this URL and accepted it only because its
    // body opens as #EXTM3U — it is definitively HLS. Re-deriving
    // the kind from the URL's shape and a HEAD probe classified it
    // MP4 when the CDN rejects HEAD, routing the renderer through
    // <video> and /file.mp4 instead of hls.js and manifest
    // rewriting: a playback break on a stream the resolve just
    // validated.
    let tmp = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("hist")).expect("mkdir hist");
    let anidb = stub_anidb_opaque().await;
    let result = run_play_assertion(tmp.path(), &anidb.uri()).await;
    result.expect("the validated playlist plays as HLS");
}

#[tokio::test]
async fn play_endpoint_resolves_natively_and_returns_session() {
    let tmp = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("hist")).expect("mkdir hist");

    let anidb = stub_anidb().await;

    let result = run_play_assertion(tmp.path(), &anidb.uri()).await;
    result.expect("play assertion succeeded");

    // The whole resolution was served by the stub: browse, episodes,
    // and languages were each hit at least once — and ani_cli_path
    // points at a file that does not exist, so a subprocess spawn
    // could not have produced the session.
    let requests = anidb
        .received_requests()
        .await
        .expect("stub recorded its requests");
    for needle in ["/browse", "/episodes", "/languages"] {
        assert!(
            requests.iter().any(|r| r.url.path().contains(needle)),
            "the native flow must hit {needle}; saw {:?}",
            requests
                .iter()
                .map(|r| r.url.path().to_string())
                .collect::<Vec<_>>()
        );
    }
}

async fn run_play_assertion(tmp: &std::path::Path, anidb_base: &str) -> Result<(), String> {
    let router = build_api_router(Arc::new(build_state(tmp, anidb_base)));
    let body = r#"{"title":"test","episode":"1","mode":"sub","quality":"best","episode_count":12}"#;
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/play")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("oneshot");

    let status = response.status();
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|e| format!("collect body: {e}"))?
        .to_bytes();
    if status != StatusCode::OK {
        let body_str = String::from_utf8_lossy(&body_bytes);
        return Err(format!("expected 200, got {status}; body: {body_str}"));
    }
    // CreateSessionResponse is Serialize-only on purpose (the
    // backend produces it; nobody parses it on the Rust side).
    // Asserting via serde_json::Value keeps that contract intact.
    let resp: serde_json::Value =
        serde_json::from_slice(&body_bytes).map_err(|e| format!("parse body: {e}"))?;
    let media_url = resp
        .get("media_url")
        .and_then(|v| v.as_str())
        .ok_or("response missing media_url")?;
    let session_id = resp
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or("response missing session_id")?;
    let media_kind = resp
        .get("media_kind")
        .and_then(|v| v.as_str())
        .ok_or("response missing media_kind")?;
    // The stub's embed resolves to a master playlist, so the kind is
    // hls and the proxy URL points at /master.m3u8.
    if media_kind != "hls" {
        return Err(format!("expected media_kind=hls, got {media_kind}"));
    }
    if !media_url.contains("/s/") || !media_url.ends_with("/master.m3u8") {
        return Err(format!("unexpected media_url shape: {media_url}"));
    }
    if session_id.is_empty() {
        return Err("session_id was empty".into());
    }
    Ok(())
}
