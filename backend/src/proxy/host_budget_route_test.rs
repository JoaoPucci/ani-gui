//! The proxy admits a segment fetch through the host's budget: a burst
//! passes at once, and the request after it waits a refill. Mounted by
//! `#[path]` beside the other proxy tests.

use std::time::Duration;

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

#[tokio::test]
async fn a_playlist_fetch_spends_the_budget_too() {
    // The host counts every request, and playlists come from it like
    // segments do: a master fetched past the burst waits a refill.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/master.m3u8"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("#EXTM3U\n#EXT-X-VERSION:3\n#EXTINF:4,\nseg.ts\n"),
        )
        .mount(&server)
        .await;
    let (router, id, _secret) = proxy_on(&format!("{}/master.m3u8", server.uri()));
    let uri = format!("/s/{}/master.m3u8", id.as_string());
    let start = tokio::time::Instant::now();
    for _ in 0..host_budget::SEGMENT_BURST {
        assert_eq!(get(router.clone(), &uri).await, StatusCode::OK);
    }
    let burst_done = tokio::time::Instant::now();
    assert!(
        burst_done - start < host_budget::SEGMENT_REFILL,
        "the burst is served without waiting"
    );
    assert_eq!(get(router.clone(), &uri).await, StatusCode::OK);
    assert!(
        tokio::time::Instant::now() - burst_done
            >= host_budget::SEGMENT_REFILL - Duration::from_millis(100),
        "the playlist fetch past the burst waited a refill"
    );
}

#[tokio::test]
async fn a_subtitle_track_fetch_spends_the_budget_too() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/en.vtt"))
        .respond_with(ResponseTemplate::new(200).set_body_string("WEBVTT\n\n"))
        .mount(&server)
        .await;
    let sessions = SessionTable::new();
    let mut session = StreamSession::new_with_kind(
        url::Url::parse("https://cdn.example/master.m3u8").expect("master url"),
        MediaKind::Hls,
        "https://embed.example/".to_string(),
    );
    session.subtitles = vec![crate::scraper::provider::SubtitleTrack {
        lang: "en".into(),
        label: "English".into(),
        default: true,
        url: format!("{}/en.vtt", server.uri()),
    }];
    let id = session.id;
    sessions.insert(session);
    let router = build_router(ProxyState {
        sessions,
        secret: AppSecret::from_bytes([7u8; 32]),
        client: reqwest::Client::new(),
        origin: ProxyOrigin::new("127.0.0.1", 1),
    });
    let uri = format!("/s/{}/sub/0.vtt", id.as_string());
    let start = tokio::time::Instant::now();
    for _ in 0..host_budget::SEGMENT_BURST {
        assert_eq!(get(router.clone(), &uri).await, StatusCode::OK);
    }
    let burst_done = tokio::time::Instant::now();
    assert!(
        burst_done - start < host_budget::SEGMENT_REFILL,
        "the burst is served without waiting"
    );
    assert_eq!(get(router.clone(), &uri).await, StatusCode::OK);
    assert!(
        tokio::time::Instant::now() - burst_done
            >= host_budget::SEGMENT_REFILL - Duration::from_millis(100),
        "the track fetch past the burst waited a refill"
    );
}

#[tokio::test]
async fn a_media_playlist_fetch_through_the_segment_route_spends_the_budget_too() {
    // A media playlist reaches the host through the segment route's
    // manifest branch; it is charged like the segments that follow it.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/index.m3u8"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("#EXTM3U\n#EXT-X-VERSION:3\n#EXTINF:4,\nseg.ts\n#EXT-X-ENDLIST\n"),
        )
        .mount(&server)
        .await;
    let (router, id, secret) = proxy_on("https://cdn.example/master.m3u8");
    let playlist = format!("{}/index.m3u8", server.uri());
    let token = sign_segment(&secret, id, &playlist);
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(playlist.as_bytes());
    let uri = format!("/s/{}/seg?u={encoded}&t={token}", id.as_string());
    let start = tokio::time::Instant::now();
    for _ in 0..host_budget::SEGMENT_BURST {
        assert_eq!(get(router.clone(), &uri).await, StatusCode::OK);
    }
    let burst_done = tokio::time::Instant::now();
    assert!(
        burst_done - start < host_budget::SEGMENT_REFILL,
        "the burst is served without waiting"
    );
    assert_eq!(get(router.clone(), &uri).await, StatusCode::OK);
    assert!(
        tokio::time::Instant::now() - burst_done
            >= host_budget::SEGMENT_REFILL - Duration::from_millis(100),
        "the media playlist fetch past the burst waited a refill"
    );
}
