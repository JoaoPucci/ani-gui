//! Tests for `crate::api::account`. Extracted via `#[path]` so the
//! route + extractor wiring complexity doesn't push `account.rs`'s
//! CCN past the ratchet — per `project_crap_inline_test_gotcha`.

use super::*;
use axum::http::header::AUTHORIZATION;

#[test]
fn parse_provider_accepts_known_slugs() {
    assert_eq!(parse_provider("anilist").unwrap(), ProviderKind::AniList);
    assert_eq!(parse_provider("mal").unwrap(), ProviderKind::MyAnimeList);
    assert_eq!(parse_provider("inhouse").unwrap(), ProviderKind::InHouse);
}

#[test]
fn parse_provider_rejects_unknown_slugs() {
    assert!(matches!(parse_provider(""), Err(AniError::Metadata)));
    assert!(matches!(parse_provider("anil"), Err(AniError::Metadata)));
    assert!(matches!(parse_provider("AniList"), Err(AniError::Metadata)));
}

#[test]
fn bearer_from_headers_extracts_token() {
    let mut h = HeaderMap::new();
    h.insert(AUTHORIZATION, "Bearer abc123".parse().unwrap());
    assert_eq!(bearer_from_headers(&h).unwrap(), "abc123");
}

#[test]
fn bearer_from_headers_rejects_missing() {
    let h = HeaderMap::new();
    assert!(matches!(
        bearer_from_headers(&h),
        Err(AniError::InvalidToken)
    ));
}

#[test]
fn bearer_from_headers_rejects_wrong_scheme() {
    let mut h = HeaderMap::new();
    h.insert(AUTHORIZATION, "Basic abc".parse().unwrap());
    assert!(matches!(
        bearer_from_headers(&h),
        Err(AniError::InvalidToken)
    ));
}

#[test]
fn bearer_from_headers_rejects_empty_token() {
    let mut h = HeaderMap::new();
    h.insert(AUTHORIZATION, "Bearer ".parse().unwrap());
    assert!(matches!(
        bearer_from_headers(&h),
        Err(AniError::InvalidToken)
    ));
}

#[test]
fn pkce_wire_round_trips() {
    let wire = PkceWire {
        verifier: "v".into(),
        challenge: "c".into(),
        method: "plain".into(),
    };
    let p = wire.into_pkce().unwrap();
    assert_eq!(p.method, PkceMethod::Plain);
    assert_eq!(p.verifier, "v");
    assert_eq!(p.challenge, "c");
}

#[test]
fn pkce_wire_rejects_unknown_method() {
    let wire = PkceWire {
        verifier: "v".into(),
        challenge: "c".into(),
        method: "s256".into(), // lowercase — must be S256
    };
    assert!(wire.into_pkce().is_none());
}

#[test]
fn bearer_from_headers_accepts_extra_whitespace_after_scheme() {
    use axum::http::HeaderMap;
    let mut h = HeaderMap::new();
    h.insert(AUTHORIZATION, "Bearer    spaced-token  ".parse().unwrap());
    assert_eq!(bearer_from_headers(&h).unwrap(), "spaced-token");
}

#[test]
fn tokens_response_round_trip_from_tokens() {
    // From<Tokens> for TokensResponse pins the wire shape. Codex P2
    // #3369941703 wired bearer auth onto get_cached_list /
    // delete_list_cache; this test pins that the shared response
    // shape used by the connect flow doesn't accidentally drop the
    // refresh_token / expires_at_epoch_s on the way out.
    let t = crate::commands::account::tokens_from_bearer("xyz");
    let resp: TokensResponse = t.into();
    let s = serde_json::to_string(&resp).unwrap();
    assert!(s.contains("\"access_token\":\"xyz\""));
    assert!(s.contains("\"refresh_token\":null"));
    assert!(s.contains("\"expires_at_epoch_s\":0"));
}

#[test]
fn auth_url_request_deserialise() {
    // Pin the wire shape from the renderer side — the renderer's
    // PkceWire JSON matches the backend's expectation.
    let body = r#"{
        "state": "csrf-token",
        "pkce": { "verifier": "v", "challenge": "c", "method": "plain" }
    }"#;
    let req: AuthUrlRequest = serde_json::from_str(body).unwrap();
    assert_eq!(req.state, "csrf-token");
    assert_eq!(req.pkce.method, "plain");
}

#[test]
fn exchange_code_request_deserialise() {
    let body = r#"{
        "code": "auth-code",
        "pkce": { "verifier": "v", "challenge": "v", "method": "plain" }
    }"#;
    let req: ExchangeCodeRequest = serde_json::from_str(body).unwrap();
    assert_eq!(req.code, "auth-code");
}

#[test]
fn auth_url_response_serialise_round_trip() {
    let r = AuthUrlResponse {
        url: "https://anilist.co/x".into(),
    };
    let s = serde_json::to_string(&r).unwrap();
    assert!(s.contains("anilist.co/x"));
}

#[test]
fn disconnect_fallback_query_defaults_to_none_when_field_missing() {
    let q: DisconnectFallbackQuery = serde_urlencoded::from_str("").unwrap();
    assert!(q.fallback_user_id.is_none());
}

#[test]
fn disconnect_fallback_query_extracts_user_id() {
    let q: DisconnectFallbackQuery = serde_urlencoded::from_str("fallback_user_id=u7").unwrap();
    assert_eq!(q.fallback_user_id.as_deref(), Some("u7"));
}

// ─── Router-level tests ──────────────────────────────────────────────
//
// These exercise the handler bodies through the real axum router so
// the lines they touch get attributed to `api/account.rs` by lcov. The
// upstream-network calls (`account::me` → AniList GraphQL) can't reach
// anything in tests; we either:
//
//   - pin paths that bail BEFORE the upstream call (missing/bad
//     bearer, unknown provider slug, malformed JSON), or
//   - exercise paths whose explicit InvalidToken branch fires when
//     the upstream call returns an error (the delete fallback gate).
//
// Coverage push for the `account.rs` ratchet under Codex P2
// #3370011855 — the new security gate landed without test cover for
// the handler body, which pushed CRAP over the ceiling.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Semaphore;
use tower::ServiceExt;

use crate::account::InternalSecret;
use crate::app::{AppState, SCRAPER_CONCURRENCY};
use crate::meta::kitsu::KitsuClient;
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};

fn test_state(td: &TempDir) -> Arc<AppState> {
    Arc::new(AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        ani_cli_path: PathBuf::from("/tmp/ani-cli"),
        bash_path: None,
        bundled_bin: None,
        history_path: td.path().join("ani-hsts"),
        scraper_slots: Arc::new(Semaphore::new(SCRAPER_CONCURRENCY)),
        image_cache_dir: td.path().join("images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::new(reqwest::Client::new()),
        config_path: td.path().join("config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: InternalSecret::from_hex_for_test("dead").unwrap(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
    })
}

async fn body_text(resp: axum::response::Response) -> String {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn post_auth_url_rejects_unknown_provider() {
    let td = TempDir::new().unwrap();
    let r = router()
        .with_state(test_state(&td))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/account/auth-url/unknown")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"state":"x","pkce":{"verifier":"v","challenge":"c","method":"plain"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    // parse_provider returns Metadata, which IntoResponse maps to a
    // structured error response — the exact status varies by error
    // kind, but it's never 2xx.
    assert!(!r.status().is_success());
}

#[tokio::test]
async fn post_auth_url_rejects_invalid_pkce_method() {
    let td = TempDir::new().unwrap();
    let r = router()
        .with_state(test_state(&td))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/account/auth-url/anilist")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"state":"x","pkce":{"verifier":"v","challenge":"c","method":"junk"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(!r.status().is_success());
}

#[tokio::test]
async fn post_auth_url_builds_anilist_url_for_plain_pkce() {
    let td = TempDir::new().unwrap();
    let r = router()
        .with_state(test_state(&td))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/account/auth-url/anilist")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"state":"csrf","pkce":{"verifier":"v","challenge":"c","method":"plain"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = body_text(r).await;
    assert!(body.contains("anilist.co"), "got: {body}");
}

#[tokio::test]
async fn post_me_requires_bearer() {
    let td = TempDir::new().unwrap();
    let r = router()
        .with_state(test_state(&td))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/account/me/anilist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(!r.status().is_success());
}

#[tokio::test]
async fn post_list_requires_bearer() {
    let td = TempDir::new().unwrap();
    let r = router()
        .with_state(test_state(&td))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/account/list/anilist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(!r.status().is_success());
}

#[tokio::test]
async fn get_cached_list_requires_bearer() {
    let td = TempDir::new().unwrap();
    let r = router()
        .with_state(test_state(&td))
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/account/list/anilist/cached")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(!r.status().is_success());
}

#[tokio::test]
async fn delete_list_cache_requires_bearer() {
    let td = TempDir::new().unwrap();
    let r = router()
        .with_state(test_state(&td))
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/account/list/anilist/cache")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(!r.status().is_success());
}

#[test]
fn me_failure_allows_fallback_for_invalid_token() {
    assert!(me_failure_allows_renderer_fallback(&AniError::InvalidToken));
}

#[test]
fn me_failure_allows_fallback_for_network_outage() {
    // Codex P2 #3370096596: offline disconnects must still be able to
    // clear the local cache rows docs/PRIVACY.md promises to drop.
    assert!(me_failure_allows_renderer_fallback(&AniError::Network));
}

#[test]
fn me_failure_allows_fallback_for_upstream_5xx() {
    // Codex P2 #3370096596: a transient AniList 5xx during disconnect
    // shouldn't strand the cache rows.
    assert!(me_failure_allows_renderer_fallback(&AniError::Upstream {
        status: 503
    }));
}

#[test]
fn me_failure_rejects_fallback_for_other_variants() {
    // Anything else (Io, Metadata, etc.) is a real backend / data
    // bug, not a renderer-driven retry signal — propagate as-is.
    assert!(!me_failure_allows_renderer_fallback(&AniError::Io));
    assert!(!me_failure_allows_renderer_fallback(&AniError::Metadata));
}

/// Codex P2 #3372942241: when a connected user is offline or
/// AniList is throwing 5xx, the cached endpoint must still serve
/// the local rows it was added to provide (Watch Later rail).
/// Mirror the disconnect fallback: try `me()` first for upstream-
/// validated identity, but when it fails for offline/401/5xx fall
/// back to the renderer-supplied user_id gated by the internal
/// secret. Without that, cached consumers lose their list at
/// exactly the moment the upstream round-trip is unavailable —
/// the opposite of what a local cache is for.
#[tokio::test]
async fn get_cached_list_serves_rows_when_offline_with_secret_fallback() {
    use crate::account::cache;
    use crate::account::provider::{ListEntry, ProviderKind, ProviderMediaId};
    use crate::account::status::ListStatus;

    let td = TempDir::new().unwrap();
    let state = test_state(&td);
    let row = ListEntry {
        provider: ProviderKind::AniList,
        media_id: ProviderMediaId(11_061),
        mal_id: Some(11_061),
        status: ListStatus::Watching,
        progress_episodes: 5,
        score_0_to_100: None,
        updated_at_epoch_s: 1_700_000_000,
        title: "Hunter x Hunter".to_string(),
    };
    cache::write_entries(&state.cache_pool, ProviderKind::AniList, "u7", &[row]).unwrap();

    let r = router()
        .with_state(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/account/list/anilist/cached?fallback_user_id=u7")
                .header("authorization", "Bearer not-a-real-bearer")
                .header("x-ani-gui-internal-secret", "dead")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(r.status().is_success(), "status was {}", r.status());
    let body = body_text(r).await;
    assert!(
        body.contains("11061"),
        "cached row missing from body: {body}"
    );
}

#[tokio::test]
async fn delete_list_cache_fallback_rejects_when_secret_header_missing() {
    // Codex P2 #3370011855: the disconnect-after-expiry fallback
    // requires the per-process internal secret. A cross-origin tab
    // can send `Bearer garbage` plus a guessed user_id under
    // permissive CORS, but it can't know the 32 bytes of entropy.
    let td = TempDir::new().unwrap();
    let r = router()
        .with_state(test_state(&td))
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/account/list/anilist/cache?fallback_user_id=u7")
                .header("authorization", "Bearer not-a-real-bearer")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(!r.status().is_success());
}

#[tokio::test]
async fn delete_list_cache_fallback_rejects_when_secret_header_wrong() {
    let td = TempDir::new().unwrap();
    let r = router()
        .with_state(test_state(&td))
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/account/list/anilist/cache?fallback_user_id=u7")
                .header("authorization", "Bearer not-a-real-bearer")
                .header("x-ani-gui-internal-secret", "wrong-value")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(!r.status().is_success());
}
