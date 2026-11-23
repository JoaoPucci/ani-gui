//! The API answers only requests carrying the launch's secret, and its
//! CORS answers only the renderer's origins.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use proptest::prelude::*;
use tempfile::TempDir;
use tower::ServiceExt;

use super::*;
use crate::account::INTERNAL_SECRET_HEADER;
use crate::api::build_api_router;
use crate::api::tests::test_app_state;

const SECRET: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn secret() -> InternalSecret {
    InternalSecret::from_hex_for_test(SECRET).expect("hex")
}

fn state(td: &TempDir) -> Arc<AppState> {
    let mut s = test_app_state(td);
    s.internal_secret = secret();
    Arc::new(s)
}

fn with_header(value: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(INTERNAL_SECRET_HEADER, value.parse().expect("header"));
    h
}

async fn send(router: axum::Router, req: Request<Body>) -> Response {
    router.oneshot(req).await.expect("router answers")
}

async fn body_bytes(resp: Response) -> Vec<u8> {
    resp.into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes()
        .to_vec()
}

fn request(method: &Method, uri: &str, body: &'static str) -> axum::http::request::Builder {
    let b = Request::builder().method(method.clone()).uri(uri);
    if body.is_empty() {
        b
    } else {
        b.header(header::CONTENT_TYPE, "application/json")
    }
}

/// A representative route of every module the API router mounts:
/// the routes in `api/mod.rs` itself across its kinds (reads, writes,
/// deletes, the progress streams, the image), and one or more of each
/// merged router (account, airing, franchise, refill, watch-order) and of the
/// modules with handlers of their own (flags, update, syncplay,
/// link_rules).
const ROUTES: &[(&str, &str, &str)] = &[
    ("GET", "/api/app-info", ""),
    ("GET", "/api/proxy-base-url", ""),
    ("GET", "/api/history", ""),
    ("DELETE", "/api/history", ""),
    ("DELETE", "/api/history/some-show", ""),
    ("POST", "/api/history/by-kitsu", r#"{"ids":[]}"#),
    ("GET", "/api/settings", ""),
    ("PUT", "/api/settings", "{}"),
    ("POST", "/api/kitsu/search", r#"{"query":"x"}"#),
    ("GET", "/api/kitsu/trending-anilist", ""),
    ("GET", "/api/image?url=https%3A%2F%2Fe.example%2Fa.jpg", ""),
    ("GET", "/api/play/stream?title=a&episode=1&mode=sub", ""),
    ("GET", "/api/download/stream?title=a&episode=1&mode=sub", ""),
    ("POST", "/api/play", "{}"),
    ("POST", "/api/play/external", "{}"),
    ("POST", "/api/play/syncplay", "{}"),
    ("POST", "/api/availability", "{}"),
    ("POST", "/api/availability/batch", "{}"),
    ("DELETE", "/api/cache", ""),
    ("DELETE", "/api/cache/images", ""),
    ("POST", "/api/history/card-lines", r#"{"rows":[]}"#),
    ("GET", "/api/flags", ""),
    ("GET", "/api/update-check", ""),
    ("GET", "/api/watched-at", ""),
    ("GET", "/api/download/default-dir", ""),
    ("GET", "/api/kitsu/airing/1", ""),
    ("GET", "/api/kitsu/franchise/1", ""),
    (
        "POST",
        "/api/watch-order/1/show",
        r#"{"today":"2026-11-02"}"#,
    ),
    (
        "POST",
        "/api/watch-order/rail",
        r#"{"rows":[],"history":[],"matches":{},"series":{}}"#,
    ),
    ("GET", "/api/kitsu/anime/1", ""),
    ("POST", "/api/refills/wait", r#"{"tickets":[]}"#),
    ("POST", "/api/account/auth-url/anilist", "{}"),
    ("GET", "/api/account/list/anilist/cached", ""),
    ("DELETE", "/api/account/list/anilist/cache/all", ""),
    ("GET", "/api/no-such-route", ""),
];

#[tokio::test]
async fn every_route_refuses_a_request_without_the_secret() {
    let td = TempDir::new().expect("tempdir");
    let state = state(&td);
    for (method, uri, body) in ROUTES {
        let method: Method = method.parse().expect("method");
        let resp = send(
            build_api_router(state.clone()),
            request(&method, uri, body)
                .body(Body::from(*body))
                .expect("request"),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{method} {uri}");
        assert!(
            body_bytes(resp).await.is_empty(),
            "{method} {uri}: a refusal has no body"
        );
    }
}

#[tokio::test]
async fn every_route_refuses_a_wrong_secret() {
    let td = TempDir::new().expect("tempdir");
    let state = state(&td);
    let wrong = "f".repeat(64);
    for (method, uri, body) in ROUTES {
        let method: Method = method.parse().expect("method");
        let resp = send(
            build_api_router(state.clone()),
            request(&method, uri, body)
                .header(INTERNAL_SECRET_HEADER, wrong.as_str())
                .body(Body::from(*body))
                .expect("request"),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{method} {uri}");
    }
}

/// A refused write does nothing: the history the request asked to
/// clear is still there.
#[tokio::test]
async fn a_refused_request_never_reaches_its_handler() {
    let td = TempDir::new().expect("tempdir");
    let state = state(&td);
    let row = "2026-01-01T00:00:00Z\t3\tshow-1\tShow One\n";
    std::fs::write(&state.history_path, row).expect("write history");
    let resp = send(
        build_api_router(state.clone()),
        Request::builder()
            .method("DELETE")
            .uri("/api/history")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        std::fs::read_to_string(&state.history_path).expect("history"),
        row
    );
}

#[tokio::test]
async fn the_secret_header_admits_a_request() {
    let td = TempDir::new().expect("tempdir");
    let resp = send(
        build_api_router(state(&td)),
        Request::builder()
            .uri("/api/settings")
            .header(INTERNAL_SECRET_HEADER, SECRET)
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
}

/// No route reads the secret from the query, the three an `<img>` or an
/// `EventSource` loads included: the main process adds the header to
/// those (electron/lib/secret-header.cjs), and a URL is kept where a
/// header is not — Chromium's disk cache, a log line.
#[tokio::test]
async fn no_route_takes_the_secret_in_its_query() {
    let td = TempDir::new().expect("tempdir");
    let state = state(&td);
    for uri in [
        format!("/api/image?url=http%3A%2F%2Fe.example%2Fa.jpg&secret={SECRET}"),
        format!("/api/play/stream?title=a&episode=1&mode=sub&prefetch=1&secret={SECRET}"),
        format!("/api/download/stream?title=a&episode=1&mode=sub&secret={SECRET}"),
        format!("/api/settings?secret={SECRET}"),
    ] {
        let resp = send(
            build_api_router(state.clone()),
            Request::builder()
                .uri(&uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{uri}");
    }
}

/// The header admits the routes an `<img>` or an `EventSource` loads,
/// as it does every other. The image handler answers a non-https url
/// with 400, which shows the request got past the gate.
#[tokio::test]
async fn the_header_admits_the_header_less_routes() {
    let td = TempDir::new().expect("tempdir");
    let state = state(&td);
    let resp = send(
        build_api_router(state.clone()),
        Request::builder()
            .uri("/api/image?url=http%3A%2F%2Fe.example%2Fa.jpg")
            .header(INTERNAL_SECRET_HEADER, SECRET)
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_bytes(resp).await, b"url must be https://");
    for path in ["/api/play/stream", "/api/download/stream"] {
        let resp = send(
            build_api_router(state.clone()),
            Request::builder()
                .uri(format!("{path}?title=a&episode=1&mode=sub&prefetch=1"))
                .header(INTERNAL_SECRET_HEADER, SECRET)
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_ne!(resp.status(), StatusCode::FORBIDDEN, "{path}");
    }
}

#[test]
fn admits_reads_the_header_and_nothing_else() {
    let s = secret();
    assert!(admits(&s, &with_header(SECRET)));
    assert!(!admits(&s, &HeaderMap::new()));
    assert!(!admits(&s, &with_header("00")));
}

proptest! {
    /// Nothing but the secret itself is admitted.
    #[test]
    fn only_the_secret_is_admitted(guess in "[ -~]{0,80}") {
        prop_assume!(guess != SECRET);
        let s = secret();
        let mut h = HeaderMap::new();
        if let Ok(v) = HeaderValue::from_str(&guess) {
            h.insert(INTERNAL_SECRET_HEADER, v);
        }
        prop_assert!(!admits(&s, &h));
    }
}

// — CORS ————————————————————————————————————————————————————————————

fn preflight(origin: &str) -> Request<Body> {
    Request::builder()
        .method("OPTIONS")
        .uri("/api/settings")
        .header(header::ORIGIN, origin)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .header(
            header::ACCESS_CONTROL_REQUEST_HEADERS,
            "x-ani-gui-internal-secret",
        )
        .body(Body::empty())
        .expect("request")
}

fn allowed_origin(resp: &Response) -> Option<&str> {
    resp.headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|v| v.to_str().ok())
}

/// The renderer's preflight carries no secret (a browser never sends
/// custom headers on one) and is answered, for the packaged origin
/// and the dev server's alike.
#[tokio::test]
async fn the_renderers_preflight_is_answered() {
    let td = TempDir::new().expect("tempdir");
    let state = state(&td);
    for origin in ["app://localhost", "http://localhost:5173"] {
        let resp = send(build_api_router(state.clone()), preflight(origin)).await;
        assert!(resp.status().is_success(), "{origin}: {}", resp.status());
        assert_eq!(allowed_origin(&resp), Some(origin), "{origin}");
        assert!(
            resp.headers().contains_key(header::ACCESS_CONTROL_MAX_AGE),
            "{origin}: preflights are cached"
        );
    }
}

#[tokio::test]
async fn a_foreign_origin_is_not_allowed() {
    let td = TempDir::new().expect("tempdir");
    let state = state(&td);
    for origin in [
        "https://evil.example",
        "null",
        "http://localhost.evil.example",
    ] {
        let resp = send(build_api_router(state.clone()), preflight(origin)).await;
        assert_eq!(allowed_origin(&resp), None, "{origin}: preflight");

        let resp = send(
            build_api_router(state.clone()),
            Request::builder()
                .uri("/api/settings")
                .header(header::ORIGIN, origin)
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{origin}");
        assert_eq!(allowed_origin(&resp), None, "{origin}");
    }
}

/// The renderer reads what it is answered, a refusal included.
#[tokio::test]
async fn the_renderer_reads_its_answers() {
    let td = TempDir::new().expect("tempdir");
    let state = state(&td);
    let resp = send(
        build_api_router(state.clone()),
        Request::builder()
            .uri("/api/settings")
            .header(header::ORIGIN, "app://localhost")
            .header(INTERNAL_SECRET_HEADER, SECRET)
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(allowed_origin(&resp), Some("app://localhost"));

    let resp = send(
        build_api_router(state),
        Request::builder()
            .uri("/api/settings")
            .header(header::ORIGIN, "app://localhost")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(allowed_origin(&resp), Some("app://localhost"));
}

// — The stream proxy beside it ——————————————————————————————————————

/// The routers as the binary serves them: the proxy's and the API's
/// merged. The proxy's routes are guarded by their session id, not
/// the secret — the media element and the download tools send none.
#[tokio::test]
async fn the_proxy_serves_its_sessions_without_the_secret() {
    let td = TempDir::new().expect("tempdir");
    let state = state(&td);
    let session = crate::proxy::StreamSession::new_with_kind(
        url::Url::parse("https://e.example/master.m3u8").expect("url"),
        crate::proxy::MediaKind::Hls,
        "",
    );
    let id = state.sessions.insert(session);
    let router =
        crate::proxy::build_router(state.proxy_state()).merge(build_api_router(state.clone()));

    let resp = send(
        router.clone(),
        Request::builder()
            .uri(format!("/s/{}/subtitles", id.as_string()))
            .header(header::ORIGIN, "app://localhost")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(allowed_origin(&resp), Some("app://localhost"));

    let unknown = crate::proxy::SessionId::new();
    let resp = send(
        router.clone(),
        Request::builder()
            .uri(format!("/s/{}/subtitles", unknown.as_string()))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "an unknown session is refused"
    );

    let resp = send(
        router,
        Request::builder()
            .uri("/api/settings")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "the API beside it is gated"
    );
}

#[tokio::test]
async fn the_proxy_allows_only_the_renderers_origin() {
    let td = TempDir::new().expect("tempdir");
    let state = state(&td);
    let id = state
        .sessions
        .insert(crate::proxy::StreamSession::new_with_kind(
            url::Url::parse("https://e.example/master.m3u8").expect("url"),
            crate::proxy::MediaKind::Hls,
            "",
        ));
    let router = crate::proxy::build_router(state.proxy_state());
    for (origin, allowed) in [
        ("app://localhost", true),
        ("http://127.0.0.1:5173", true),
        ("https://evil.example", false),
    ] {
        let resp = send(
            router.clone(),
            Request::builder()
                .uri(format!("/s/{}/subtitles", id.as_string()))
                .header(header::ORIGIN, origin)
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK, "{origin}");
        assert_eq!(allowed_origin(&resp), allowed.then_some(origin), "{origin}");
    }
}
