//! The proxy admits a segment fetch through the host's budget: a burst
//! passes at once, and the request after it waits a refill. Mounted by
//! `#[path]` beside the other proxy tests.

use super::*;
use tower::ServiceExt as _;
use wiremock::matchers::{method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn proxy_on(master: &str) -> (Router, SessionId, AppSecret) {
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
        sessions,
        secret: secret.clone(),
        client: reqwest::Client::new(),
        origin: ProxyOrigin::new("127.0.0.1", 1),
    };
    (build_router(state), id, secret)
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
async fn the_request_after_the_burst_waits_a_refill() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/seg.ts"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"payload".to_vec()))
        .mount(&server)
        .await;
    let (router, id, secret) = proxy_on("https://cdn.example/master.m3u8");
    let seg = format!("{}/seg.ts", server.uri());
    let token = sign_segment(&secret, id, &seg);
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(seg.as_bytes());
    let uri = format!("/s/{}/seg?u={encoded}&t={token}", id.as_string());
    let start = tokio::time::Instant::now();
    for _ in 0..host_budget::SEGMENT_BURST {
        assert_eq!(get(router.clone(), &uri).await, StatusCode::OK);
    }
    let burst_done = tokio::time::Instant::now();
    assert!(
        burst_done - start < host_budget::SEGMENT_REFILL,
        "the burst is served without waiting: {:?}",
        burst_done - start
    );
    assert_eq!(get(router.clone(), &uri).await, StatusCode::OK);
    let after = tokio::time::Instant::now();
    assert!(
        after - burst_done >= host_budget::SEGMENT_REFILL - Duration::from_millis(100),
        "the request after the burst waited a refill: {:?}",
        after - burst_done
    );
}
