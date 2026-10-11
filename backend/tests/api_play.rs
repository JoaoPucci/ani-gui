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
//! `AppState::providers`. It has to be — a live resolve inside an
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
use ani_gui::provider_set::ProviderSet;
use ani_gui::proxy::{AppSecret, ProxyOrigin, SessionTable};
use ani_gui::scraper::provider::ProviderId;
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

/// Build an `AppState` whose provider bases point at local stubs.
/// There is no script path to neutralise any more — the field went
/// with the subprocess — so what keeps this honest is that the stubs
/// are the only things serving the walk, and the assertions below
/// check they were asked for every step. Without a hianime origin
/// the order stops at anidb: a test that stubs one provider must not
/// fall through to the real other one.
fn build_state(tmp: &std::path::Path, anidb_base: &str, hianime_base: Option<&str>) -> AppState {
    let provider_order = match hianime_base {
        Some(_) => vec![ProviderId::Anidb, ProviderId::Hianime],
        None => vec![ProviderId::Anidb],
    };
    let mut providers =
        ProviderSet::listing(provider_order).with_origin(ProviderId::Anidb, anidb_base);
    if let Some(base) = hianime_base {
        providers.set_origin(ProviderId::Hianime, base);
    }
    AppState {
        providers,
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: ani_gui::legacy_script::SweepReport::default(),
        history_path: tmp.join("hist/history"),
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

/// An anidb that is down the way it went down in September 2026: a
/// maintenance page on every route, the JSON API included.
async fn stub_anidb_down() -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(
            wiremock::ResponseTemplate::new(503).set_body_string("<h1>Under Maintenance</h1>"),
        )
        .mount(&server)
        .await;
    server
}

/// The embed page's payload as the site ships it: the player JSON
/// XOR'd under its versioned key, then base64'd.
fn embed_blob(payload: &str) -> String {
    use base64::Engine as _;
    const KEY: &[u8] = b"otaku-embed-v1";
    let xored: Vec<u8> = payload
        .bytes()
        .zip(KEY.iter().cycle())
        .map(|(b, k)| b ^ k)
        .collect();
    base64::engine::general_purpose::STANDARD.encode(xored)
}

/// A local hianime stub covering its whole flow: search page →
/// episode list → server list → embed page → master. The embed lives
/// on the stub's own origin, so every request stays on the machine.
async fn stub_hianime() -> wiremock::MockServer {
    use base64::Engine as _;
    let server = wiremock::MockServer::start().await;
    let base = server.uri();
    let search = format!(
        r#"<div class="film_list-wrap"><div class="flw-item"><div class="film-detail"><h3 class="film-name"><a href="{base}/test-show-1" title="test" class="dynamic-name">test</a></h3><div class="fd-infor"><span class="fdi-item">TV</span></div></div></div></div><div id="main-sidebar"></div>"#
    );
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/search"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(search))
        .mount(&server)
        .await;
    let eps: Vec<String> = (1..=12)
        .map(|n| {
            format!(
                r#"<a class="ep-item" data-number="{n}" data-id="{}"></a>"#,
                1000 + n
            )
        })
        .collect();
    let list = serde_json::json!({ "status": true, "html": eps.join("") }).to_string();
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/theme/episode/list/1"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(list))
        .mount(&server)
        .await;
    let hash =
        base64::engine::general_purpose::STANDARD.encode(format!("{base}/stream/mal/1/1/sub"));
    let servers = serde_json::json!({
        "status": true,
        "html": format!(r#"<div class="server-item" data-type="sub" data-server-name="HD-1" data-hash="{hash}"></div>"#),
    })
    .to_string();
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/theme/episode/servers"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(servers))
        .mount(&server)
        .await;
    let payload =
        serde_json::json!({ "src": format!("{base}/op/master.m3u8"), "subtitles": [] }).to_string();
    let embed = format!(
        r#"<html><body><script>window.__P="{}"</script></body></html>"#,
        embed_blob(&payload)
    );
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/stream/mal/1/1/sub"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(embed))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/op/master.m3u8"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("#EXTM3U\n"))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn play_fails_over_to_the_next_provider_when_the_first_is_down() {
    let tmp = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("hist")).expect("mkdir hist");
    let anidb = stub_anidb_down().await;
    let hianime = stub_hianime().await;
    let state = Arc::new(build_state(tmp.path(), &anidb.uri(), Some(&hianime.uri())));
    // One failure short of open: the outage this play observes is
    // what opens the primary's breaker, and only the primary's.
    for _ in 0..ani_gui::scraper::gate::FAILURE_THRESHOLD - 1 {
        state.providers.gate(ProviderId::Anidb).record(
            ani_gui::scraper::outcome::ScrapeOutcome::Failure,
            tokio::time::Instant::now(),
        );
    }

    run_play_assertion(state.clone())
        .await
        .expect("the play resolved through the fallback");

    assert!(
        !anidb
            .received_requests()
            .await
            .expect("stub recorded its requests")
            .is_empty(),
        "the primary was asked first"
    );
    let asked: Vec<String> = hianime
        .received_requests()
        .await
        .expect("stub recorded its requests")
        .iter()
        .map(|r| r.url.path().to_string())
        .collect();
    for needle in [
        "/search",
        "/api/theme/episode/list/1",
        "/api/theme/episode/servers",
        "/stream/mal/1/1/sub",
    ] {
        assert!(
            asked.iter().any(|p| p.contains(needle)),
            "the fallback's flow must hit {needle}; saw {asked:?}"
        );
    }
    // The row is keyed on the fallback's qualified id — the bare slug
    // would be read back as anidb's.
    let history =
        std::fs::read_to_string(tmp.path().join("hist/history")).expect("history written");
    assert!(
        history.contains("hianime:test-show-1"),
        "history keyed on the answering provider: {history}"
    );
    assert!(
        state.providers.gate(ProviderId::Anidb).is_open(),
        "the primary's outage landed on its own breaker"
    );
    assert!(
        !state.providers.gate(ProviderId::Hianime).is_open(),
        "and not on the provider that answered"
    );
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
    let state = Arc::new(build_state(tmp.path(), &anidb.uri(), None));
    let result = run_play_assertion(state).await;
    result.expect("the validated playlist plays as HLS");
}

#[tokio::test]
async fn play_endpoint_resolves_natively_and_returns_session() {
    let tmp = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("hist")).expect("mkdir hist");

    let anidb = stub_anidb().await;

    let state = Arc::new(build_state(tmp.path(), &anidb.uri(), None));
    let result = run_play_assertion(state).await;
    result.expect("play assertion succeeded");

    // The whole resolution was served by the stub: browse, episodes,
    // and languages were each hit at least once, which is what says
    // the session came from the native walk rather than anywhere
    // else.
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

async fn run_play_assertion(state: Arc<AppState>) -> Result<(), String> {
    let router = build_api_router(state);
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

/// A Continue Watching row from anidb.app's days — its bare slug, the
/// Kitsu id the play recorded, a numbering stamp in the sidecar — plays
/// on hianime, the provider the app walks, and loses nothing: the play
/// writes hianime's row beside it, the old row keeps its place and its
/// episode, and the show resumes from the row the user watched last.
#[tokio::test]
async fn a_history_row_from_anidb_plays_on_hianime_and_keeps_its_place() {
    let tmp = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("hist")).expect("mkdir hist");
    let hianime = stub_hianime().await;
    let mut state = build_state(tmp.path(), "http://127.0.0.1:1", Some(&hianime.uri()));
    state.providers.set_order(vec![ProviderId::Hianime]);
    let history_path = tmp.path().join("hist/history");
    // The legacy row: a continuation anidb numbered cumulatively, its
    // episode 43 the entry's third, translated by the sidecar's stamp.
    ani_gui::history::write_atomic(
        &history_path,
        &[ani_gui::history::HistoryEntry {
            ep_no: "43".into(),
            id: "test-show-1".into(),
            title: "test".into(),
            watched_at: Some(1_700_000_000_000),
            kitsu_id: Some("777".into()),
        }],
    )
    .expect("seed history");
    std::fs::write(tmp.path().join("hist/ani-gui-offsets"), "test-show-1\t40\n")
        .expect("seed numbering");
    let state = Arc::new(state);

    let router = build_api_router(state.clone());
    let body = r#"{"title":"test","episode":"4","mode":"sub","quality":"best","episode_count":12,"kitsu_id":"777"}"#;
    let response = router
        .clone()
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
    assert_eq!(response.status(), StatusCode::OK, "the play resolves");
    // The renderer stamps the watch once the play resolves, with the
    // Kitsu id of the page it came from.
    let marked = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/play/mark-watched")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert!(marked.status().is_success(), "{}", marked.status());
    assert!(
        hianime
            .received_requests()
            .await
            .expect("recorded")
            .iter()
            .any(|r| r.url.path() == "/search"),
        "it resolved on hianime"
    );

    let rows = ani_gui::history::read_all(&history_path).expect("history");
    let legacy = rows
        .iter()
        .find(|r| r.id == "test-show-1")
        .expect("the legacy row is kept");
    assert_eq!(legacy.ep_no, "43", "untouched");
    assert_eq!(legacy.kitsu_id.as_deref(), Some("777"));
    let played = rows
        .iter()
        .find(|r| r.id == "hianime:test-show-1")
        .expect("hianime's row is written beside it");
    assert_eq!(played.kitsu_id.as_deref(), Some("777"));

    // The list still reads the legacy row in the entry's numbering.
    let listed = get_json(&router, "/api/history").await;
    let legacy_listed = listed
        .as_array()
        .expect("a list")
        .iter()
        .find(|r| r["id"] == "test-show-1")
        .expect("listed");
    assert_eq!(
        legacy_listed["ep_no"], "3",
        "the sidecar still translates it"
    );

    // And the show resumes from the row watched last: hianime's.
    let resumed = get_json(&router, "/api/history/by-kitsu/777").await;
    assert_eq!(resumed["id"], "hianime:test-show-1");
}

async fn get_json(router: &axum::Router, uri: &str) -> serde_json::Value {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("json")
}
