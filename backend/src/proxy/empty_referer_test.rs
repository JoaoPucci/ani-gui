//! What the proxy sends upstream for a session that stores no referer.
//!
//! Extracted via `#[path]` so the inline `#[cfg(test)]` module's
//! complexity doesn't pile onto `mod.rs`'s CCN budget — per
//! `project_crap_inline_test_gotcha`.
//!
//! A source whose CDN checks nothing resolves with no referer, and the
//! session stores that as the empty string — the same spelling the
//! cache's HEAD revalidation, the external player's arguments and the
//! download tools already read as "send none". Parsing alone does not
//! carry that meaning: `HeaderValue::from_str("")` succeeds, so a
//! session with no referer went upstream announcing an empty one, and
//! a header that names nobody is still a header. Each route that
//! fetches upstream builds its own headers, so each is pinned here:
//! the master playlist, a media playlist reached through the segment
//! route, a raw segment, the MP4 pass-through, and the HEAD that
//! classifies an extensionless upstream.

use super::*;
use tower::ServiceExt as _;
use wiremock::matchers::{method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A minimal master playlist naming one variant, so the master route
/// gets something `rewrite_master` accepts.
const MASTER: &str = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=800000\nv/index.m3u8\n";

/// A minimal media playlist naming one segment.
const MEDIA: &str = "#EXTM3U\n#EXT-X-TARGETDURATION:6\n#EXTINF:6.0,\nseg-001.ts\n#EXT-X-ENDLIST\n";

/// A proxy holding one session over `upstream_url` whose stored
/// referer is empty, plus that session's id and signing secret.
fn proxy_over(upstream_url: &str) -> (Router, SessionId, AppSecret) {
    let secret = AppSecret::from_bytes([9u8; 32]);
    let sessions = SessionTable::new();
    let session = StreamSession::new(
        url::Url::parse(upstream_url).expect("upstream url"),
        String::new(),
    );
    let id = session.id;
    sessions.insert(session);

    let state = ProxyState {
        sessions,
        secret: secret.clone(),
        client: reqwest::Client::new(),
        origin: ProxyOrigin::new("127.0.0.1", 1),
    };
    (build_router(state), id, secret)
}

/// The proxy URI that fetches `segment_url` on `session`.
fn seg_uri(secret: &AppSecret, session: SessionId, segment_url: &str) -> String {
    let token = sign_segment(secret, session, segment_url);
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(segment_url.as_bytes());
    format!("/s/{}/seg?u={encoded}&t={token}", session.as_string())
}

/// Drive one request through the proxy and return its status.
async fn get(router: Router, uri: String) -> StatusCode {
    router
        .oneshot(
            axum::http::Request::builder()
                .uri(uri)
                .body(axum::body::Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds")
        .status()
}

/// The referer the upstream saw, or `None` when the proxy sent none.
/// An empty header value is `Some("")` here, which is the answer this
/// file exists to refuse.
async fn referer_seen_by(server: &MockServer) -> Option<String> {
    let reqs = server
        .received_requests()
        .await
        .expect("the mock server records requests");
    let req = reqs.first().expect("the proxy fetched upstream");
    req.headers
        .get("referer")
        .map(|v| v.to_str().expect("referer is ascii").to_string())
}

#[tokio::test]
async fn a_master_fetch_without_a_referer_sends_no_referer_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/master.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(MASTER))
        .mount(&server)
        .await;

    let (router, session, _secret) = proxy_over(&format!("{}/master.m3u8", server.uri()));
    let status = get(router, format!("/s/{}/master.m3u8", session.as_string())).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        referer_seen_by(&server).await,
        None,
        "the master fetch announced an empty referer instead of none",
    );
}

#[tokio::test]
async fn a_media_playlist_fetch_without_a_referer_sends_no_referer_header() {
    // The subtitle and variant playlists a master names come back
    // through the segment route, which fetches them as text before
    // rewriting them — a different fetch from the master's own.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/subs/en.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(MEDIA))
        .mount(&server)
        .await;

    let (router, session, secret) = proxy_over("https://cdn.example/master.m3u8");
    let uri = seg_uri(&secret, session, &format!("{}/subs/en.m3u8", server.uri()));
    let status = get(router, uri).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        referer_seen_by(&server).await,
        None,
        "the media playlist fetch announced an empty referer instead of none",
    );
}

#[tokio::test]
async fn a_raw_segment_without_a_referer_sends_no_referer_header() {
    // Anything the segment route does not recognise as a playlist —
    // a .ts or .m4s chunk, a .vtt subtitle track — streams through a
    // request the route builds itself.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/subs/en.vtt"))
        .respond_with(ResponseTemplate::new(200).set_body_string("WEBVTT\n"))
        .mount(&server)
        .await;

    let (router, session, secret) = proxy_over("https://cdn.example/master.m3u8");
    let uri = seg_uri(&secret, session, &format!("{}/subs/en.vtt", server.uri()));
    let status = get(router, uri).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        referer_seen_by(&server).await,
        None,
        "the raw segment fetch announced an empty referer instead of none",
    );
}

#[tokio::test]
async fn an_mp4_pass_through_without_a_referer_sends_no_referer_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/video.mp4"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"payload".to_vec()))
        .mount(&server)
        .await;

    let (router, session, _secret) = proxy_over(&format!("{}/video.mp4", server.uri()));
    let status = get(router, format!("/s/{}/file.mp4", session.as_string())).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        referer_seen_by(&server).await,
        None,
        "the MP4 pass-through announced an empty referer instead of none",
    );
}

#[tokio::test]
async fn a_kind_probe_without_a_referer_sends_no_referer_header() {
    // The probe that classifies an upstream with no usable extension
    // reaches no route — it runs before a session exists — so it is
    // driven directly. It reads the same stored referer and had the
    // same empty-header answer.
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(wm_path("/videos/x/sub/1"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "video/mp4"))
        .mount(&server)
        .await;

    let client = upstream::build_client().expect("client builds");
    let url = Url::parse(&format!("{}/videos/x/sub/1", server.uri())).expect("probe url");
    let kind = upstream::classify_via_head(&client, &url, "")
        .await
        .expect("the probe answers");

    assert!(matches!(kind, MediaKind::Mp4));
    assert_eq!(
        referer_seen_by(&server).await,
        None,
        "the kind probe announced an empty referer instead of none",
    );
}
