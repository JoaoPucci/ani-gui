//! End-to-end integration test for the M2 play endpoints.
//!
//! Mirrors the curl-shim staging from `anicli_run_debug.rs` — stages
//! the vendored shim ahead of the system PATH, copies the canned
//! allanime fixtures into a tmp dir, then drives `POST /api/play`
//! through the full axum router. Verifies the chain
//!
//!   POST /api/play  →  run_debug spawns ani-cli (which calls our shim)
//!                  →  parse `Selected link:` from stdout
//!                  →  create_session wraps the upstream URL
//!                  →  return CreateSessionResponse
//!
//! works end-to-end. The unit tests in `api/mod.rs` cover the route's
//! body validation; this file covers the actual subprocess path.
//!
//! Linux-only for the same reason as `anicli_run_debug.rs` — `ani-cli`
//! depends on a POSIX shell + GNU userland.
//!
//! Hermetic in both directions. The shell half has always been: the
//! curl shim answers ani-cli's requests from canned fixtures. The Rust
//! half was not — the play handler runs its own allanime search to
//! disambiguate the title, and that call went to the live API, so a
//! throttled IP failed this test with a network 503 that read exactly
//! like a regression in the diff under review. The search now goes to
//! a local stub too, via `AppState::allanime_base`.

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

fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(std::path::Path::parent)
        .expect("manifest is two levels deep from repo root")
        .to_path_buf()
}

/// Stage the curl shim under both names 5.0's failover probes, with
/// CURL_FIXTURE_DIR pinned to the repo's anidb fixtures. Same
/// construction as `anicli_run_debug.rs` — kept inline rather than
/// extracted because the two test files have only this one piece in
/// common and a shared helper would couple them more than they're
/// already coupled.
fn stage_anidb_shim(tmp: &std::path::Path) -> PathBuf {
    let bin = tmp.join("bin");
    std::fs::create_dir_all(&bin).expect("mkdir bin");
    let repo = repo_root();
    let body = format!(
        "#!/bin/sh\nexport CURL_FIXTURE_DIR={fixtures}\nexec sh {repo}/tests/bash/helpers/curl_shim.sh \"$@\"\n",
        fixtures = repo.join("tests/fixtures/anidb").display(),
        repo = repo.display(),
    );
    for name in ["curl", "curl_firefox135"] {
        let dst = bin.join(name);
        std::fs::write(&dst, &body).expect("write wrapper shim");
        #[allow(unused_mut)]
        let mut perms = std::fs::metadata(&dst).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o755);
        }
        std::fs::set_permissions(&dst, perms).expect("chmod +x");
    }
    bin
}

/// Stand up a local allanime stub answering the search the play
/// handler runs before it ever spawns ani-cli.
///
/// The candidate is shaped to be picked: one hit, an episode count
/// that comfortably covers the requested episode, and the `Show`
/// typename the parser keys on. What matters for hermeticity is only
/// that the request never leaves the machine.
async fn stub_allanime() -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    let body = serde_json::json!({
        "data": {
            "shows": {
                "edges": [
                    {
                        "_id": "test-show",
                        "name": "test",
                        "availableEpisodes": {"sub": 12, "dub": 12, "raw": 0},
                        "__typename": "Show"
                    }
                ]
            }
        }
    });
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    server
}

/// Build an `AppState` pointed at the real `ani-cli` script and
/// otherwise pinned to the test's tmp dir. The test process's
/// `$PATH` should include the staged shim before this runs — the
/// play handler invokes `run_debug` with `path_override: None`, so
/// it inherits whatever PATH the test sets.
///
/// `allanime_base` points the Rust-side disambiguation search at the
/// local stub. Production leaves it `None`, which means the real API.
fn build_state(tmp: &std::path::Path, allanime_base: &str) -> AppState {
    AppState {
        allanime_base: Some(allanime_base.to_string()),
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        ani_cli_path: repo_root().join("ani-cli"),
        bash_path: None,
        bundled_bin: None,
        botan_shim_bin: None,
        history_path: tmp.join("hist/ani-hsts"),
        scraper_gate: Arc::new(ani_gui::scraper::gate::ScraperGate::new()),
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

#[tokio::test]
async fn play_endpoint_resolves_through_curl_shim_and_returns_session() {
    let tmp = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("hist")).expect("mkdir hist");

    let bin = stage_anidb_shim(tmp.path());
    let allanime = stub_allanime().await;

    // Prepend the shim dir to the test process's PATH. The play
    // handler's `run_debug` call inherits this PATH, so ani-cli's
    // `curl` invocations resolve to our shim. A previous PATH is
    // restored at the end of the test for hygiene — even though
    // each integration-test file runs in its own process, doing
    // so makes a future shared-test refactor safer.
    let saved_path = std::env::var("PATH").ok();
    let new_path = format!("{}:{}", bin.display(), saved_path.as_deref().unwrap_or(""));
    std::env::set_var("PATH", &new_path);

    let result = run_play_assertion(tmp.path(), &allanime.uri()).await;

    if let Some(p) = saved_path {
        std::env::set_var("PATH", p);
    } else {
        std::env::remove_var("PATH");
    }
    result.expect("play assertion succeeded");

    // The point of the stub: prove the disambiguation search was
    // served locally. A zero here means the handler reached past the
    // override to the real API, and this test would be one throttled
    // IP away from a false red on someone else's diff.
    let searches = allanime
        .received_requests()
        .await
        .expect("stub recorded its requests")
        .len();
    assert!(
        searches >= 1,
        "the allanime search must be served by the local stub, saw {searches} requests"
    );
}

async fn run_play_assertion(tmp: &std::path::Path, allanime_base: &str) -> Result<(), String> {
    let router = build_api_router(Arc::new(build_state(tmp, allanime_base)));
    let body = r#"{"title":"test","episode":"1","mode":"sub","quality":"best"}"#;
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
    // The shim resolves an HLS variant playlist (see
    // tests/fixtures/anidb/master_op.m3u8), so the kind is hls and
    // the proxy URL points at /master.m3u8.
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
