//! The proxy charges every fetch it makes on the player's behalf —
//! segments, playlists, subtitle tracks — to the host's budget: a burst
//! passes at once, the request after it waits a refill, and each
//! proxy's budget is its own. Mounted by `#[path]` beside the other
//! proxy tests.

use std::time::Duration;

use super::*;
use tower::ServiceExt as _;
use wiremock::matchers::{method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A proxy over `master` with the budget given, on the client the app
/// builds — the one the hop of a redirect is visible to.
fn proxy_with(
    master: &str,
    host_budget: std::sync::Arc<host_budget::HostBudget>,
) -> (Router, SessionId, AppSecret) {
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
        client: upstream::build_client().expect("client builds"),
        origin: ProxyOrigin::new("127.0.0.1", 1),
        host_budget,
    };
    (build_router(state), id, secret)
}

fn proxy_on(master: &str) -> (Router, SessionId, AppSecret) {
    proxy_with(master, host_budget::HostBudget::fresh())
}

fn on_hand(budget: &host_budget::HostBudget, server: &MockServer) -> Option<f64> {
    let url = url::Url::parse(&server.uri()).expect("server url");
    budget.on_hand(&host_budget::host_key(&url))
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
        host_budget: host_budget::HostBudget::fresh(),
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

#[tokio::test]
async fn each_proxy_owns_its_budget() {
    // The tests' mock servers are pooled and their ports recycled, so
    // a budget global to the process would hand the next router a
    // bucket the previous one spent. A budget belongs to the state it
    // was built with — the app's, in production, and each test's
    // here: a second proxy built with its own against the same host
    // starts with a full burst.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/seg.ts"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"payload".to_vec()))
        .mount(&server)
        .await;
    let seg = format!("{}/seg.ts", server.uri());
    let (first, id_a, secret_a) = proxy_on("https://cdn.example/master.m3u8");
    let uri_a = format!(
        "/s/{}/seg?u={}&t={}",
        id_a.as_string(),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(seg.as_bytes()),
        sign_segment(&secret_a, id_a, &seg)
    );
    for _ in 0..host_budget::SEGMENT_BURST {
        assert_eq!(get(first.clone(), &uri_a).await, StatusCode::OK);
    }
    let (second, id_b, secret_b) = proxy_on("https://cdn.example/master.m3u8");
    let uri_b = format!(
        "/s/{}/seg?u={}&t={}",
        id_b.as_string(),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(seg.as_bytes()),
        sign_segment(&secret_b, id_b, &seg)
    );
    let start = tokio::time::Instant::now();
    for _ in 0..host_budget::SEGMENT_BURST {
        assert_eq!(get(second.clone(), &uri_b).await, StatusCode::OK);
    }
    assert!(
        tokio::time::Instant::now() - start < host_budget::SEGMENT_REFILL,
        "the second proxy's burst is its own"
    );
}

#[tokio::test]
async fn a_redirect_hop_is_charged_to_the_host_it_lands_on() {
    // The master the session stored redirects to another server.
    // Each hop is a request the host that answers it counts: the
    // server that redirected spent a token, and so did the one that
    // served the manifest — where a transport following the redirect
    // on its own charged the first and left the second unpaced.
    let origin = MockServer::start().await;
    let target = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/master.m3u8"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", format!("{}/real.m3u8", target.uri()).as_str()),
        )
        .mount(&origin)
        .await;
    Mock::given(method("GET"))
        .and(wm_path("/real.m3u8"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("#EXTM3U\n#EXT-X-VERSION:3\n#EXTINF:4,\nseg.ts\n"),
        )
        .mount(&target)
        .await;
    let budget = host_budget::HostBudget::fresh();
    let (router, id, _secret) =
        proxy_with(&format!("{}/master.m3u8", origin.uri()), budget.clone());
    let uri = format!("/s/{}/master.m3u8", id.as_string());
    assert_eq!(get(router, &uri).await, StatusCode::OK);
    let one_spent = Some(f64::from(host_budget::SEGMENT_BURST - 1));
    assert_eq!(
        on_hand(&budget, &origin),
        one_spent,
        "the server that redirected answered a request"
    );
    assert_eq!(
        on_hand(&budget, &target),
        one_spent,
        "the server the redirect landed on answered one too"
    );
}
