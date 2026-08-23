//! What the raw-segment route sends upstream as `Referer:`.
//!
//! Extracted via `#[path]` so the inline `#[cfg(test)]` module's
//! complexity doesn't pile onto `mod.rs`'s CCN budget — per
//! `project_crap_inline_test_gotcha`.
//!
//! The session's stored referer is what the CDN checks, and the three
//! fetches that go through `upstream::` omit the header when the
//! stored string will not parse as one. The raw-segment branch built
//! its own request and substituted a literal instead, so a session
//! whose referer cannot become a header value made the proxy announce
//! a provider the app no longer talks to. That path is reachable:
//! `POST /api/sessions` validates `upstream_url` and stores `referer`
//! verbatim.

use super::*;
use tower::ServiceExt as _;
use wiremock::matchers::{method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A proxy fronting `server`, holding one session with the given
/// referer, plus a signed URL for `segment_url` on that session.
async fn proxy_for(referer: &str, segment_url: &str) -> (Router, String) {
    let secret = AppSecret::from_bytes([7u8; 32]);
    let sessions = SessionTable::new();
    let session = StreamSession::new(
        url::Url::parse("https://cdn.example/master.m3u8").expect("master url"),
        referer.to_string(),
    );
    let id = session.id;
    sessions.insert(session);

    let token = sign_segment(&secret, id, segment_url);
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(segment_url.as_bytes());
    let uri = format!("/s/{}/seg?u={encoded}&t={token}", id.as_string());

    let state = ProxyState {
        sessions,
        secret,
        client: reqwest::Client::new(),
        origin: ProxyOrigin::new("127.0.0.1", 1),
    };
    (build_router(state), uri)
}

/// The referer the upstream saw, or `None` when the proxy sent none.
async fn referer_seen_by(server: &MockServer) -> Option<String> {
    let reqs = server
        .received_requests()
        .await
        .expect("the mock server records requests");
    let req = reqs.first().expect("the proxy fetched the segment");
    req.headers
        .get("referer")
        .map(|v| v.to_str().expect("referer is ascii").to_string())
}

#[tokio::test]
async fn a_raw_segment_carries_the_session_referer() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/seg-001.ts"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"payload".to_vec()))
        .mount(&server)
        .await;

    let seg = format!("{}/seg-001.ts", server.uri());
    let (router, uri) = proxy_for("https://cdn.example/watch", &seg).await;
    let resp = router
        .oneshot(
            axum::http::Request::builder()
                .uri(uri)
                .body(axum::body::Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        referer_seen_by(&server).await.as_deref(),
        Some("https://cdn.example/watch"),
    );
}

#[tokio::test]
async fn a_referer_that_is_not_a_header_sends_none_rather_than_a_substitute() {
    // A newline cannot appear in a header value, so `HeaderValue::from_str`
    // refuses this and the branch that picks a replacement is taken. What
    // it must not do is name some other origin: announcing a provider the
    // app no longer resolves through is a worse answer than announcing
    // nothing, and it is the answer that survived the migration.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/seg-002.ts"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"payload".to_vec()))
        .mount(&server)
        .await;

    let seg = format!("{}/seg-002.ts", server.uri());
    let (router, uri) = proxy_for("https://cdn.example/wa\ntch", &seg).await;
    let resp = router
        .oneshot(
            axum::http::Request::builder()
                .uri(uri)
                .body(axum::body::Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        referer_seen_by(&server).await,
        None,
        "the raw-segment fetch substituted a referer instead of omitting it",
    );
}
