//! The routes that serve media note the fetch on the session table,
//! so a download running beside a playing stream can yield to it.
//! Mounted by `#[path]` beside the other proxy tests.

use super::*;
use tower::ServiceExt as _;
use wiremock::matchers::{method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const WINDOW: std::time::Duration = std::time::Duration::from_secs(30);

/// A proxy holding one HLS session on `master`, with the table the
/// caller can ask afterwards.
fn proxy_on(master: &str) -> (Router, SessionTable, SessionId, AppSecret) {
    let secret = AppSecret::from_bytes([7u8; 32]);
    let sessions = SessionTable::new();
    let session = StreamSession::new_with_kind(
        url::Url::parse(master).expect("master url"),
        MediaKind::Hls,
        "https://embed.example/".to_string(),
    );
    let id = session.id;
    sessions.insert(session);
    let state = ProxyState {
        sessions: sessions.clone(),
        secret: secret.clone(),
        client: reqwest::Client::new(),
        origin: ProxyOrigin::new("127.0.0.1", 1),
    };
    (build_router(state), sessions, id, secret)
}

async fn get(router: Router, uri: &str) -> StatusCode {
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

#[tokio::test]
async fn a_segment_fetch_marks_playback_live() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/seg-001.ts"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"payload".to_vec()))
        .mount(&server)
        .await;
    let (router, sessions, id, secret) = proxy_on("https://cdn.example/master.m3u8");
    assert!(!sessions.playback_live(WINDOW), "nothing served yet");
    let seg = format!("{}/seg-001.ts", server.uri());
    let token = sign_segment(&secret, id, &seg);
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(seg.as_bytes());
    let status = get(
        router,
        &format!("/s/{}/seg?u={encoded}&t={token}", id.as_string()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        sessions.playback_live(WINDOW),
        "the player is fetching media"
    );
}

#[tokio::test]
async fn a_master_fetch_marks_playback_live_before_the_upstream_answers() {
    // The master is the first thing a starting player asks for: the
    // download should yield from that moment, not after the first
    // segment has already competed with it — so the note lands when
    // the request is admitted, whatever the upstream then says.
    let (router, sessions, id, _) = proxy_on("http://127.0.0.1:9/master.m3u8");
    let _ = get(router, &format!("/s/{}/master.m3u8", id.as_string())).await;
    assert!(sessions.playback_live(WINDOW));
}

#[tokio::test]
async fn an_unknown_session_marks_nothing() {
    let (router, sessions, _, _) = proxy_on("https://cdn.example/master.m3u8");
    let stray = SessionId::new();
    let status = get(router, &format!("/s/{}/master.m3u8", stray.as_string())).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!sessions.playback_live(WINDOW));
}

#[tokio::test]
async fn a_subtitle_listing_is_not_media() {
    // Subtitles are fetched once as the player mounts; they say
    // nothing about segments flowing.
    let (router, sessions, id, _) = proxy_on("https://cdn.example/master.m3u8");
    let _ = get(router, &format!("/s/{}/subtitles", id.as_string())).await;
    assert!(!sessions.playback_live(WINDOW));
}
