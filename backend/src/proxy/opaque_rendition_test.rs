//! A master's renditions are playlists whatever their URLs look like.
//!
//! Extracted via `#[path]` so the inline `#[cfg(test)]` module's
//! complexity doesn't pile onto `mod.rs`'s CCN budget — per
//! `project_crap_inline_test_gotcha`.
//!
//! HLS names a playlist by where it appears, not by its URL: a
//! variant or an `EXT-X-MEDIA` rendition may sit at `INDEX.M3U8`, at
//! `playlist?id=720`, or at a path with no extension at all. Each one
//! the player or a relayed download asks for must come back rewritten,
//! its segments pointing at the proxy; streamed through as bytes, its
//! relative segment URIs resolve against the proxy's own address and
//! nothing behind them exists.

use super::*;
use tower::ServiceExt as _;
use wiremock::matchers::{method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MEDIA: &str = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\nseg-001.ts\n#EXT-X-ENDLIST\n";

/// A proxy holding one HLS session on `master`; its origin is the
/// prefix the rewritten manifests' URLs start with.
fn proxy_on(master: &str) -> (Router, SessionId, ProxyOrigin) {
    let sessions = SessionTable::new();
    let session = StreamSession::new_with_kind(
        url::Url::parse(master).expect("master url"),
        MediaKind::Hls,
        "https://embed.example/".to_string(),
    );
    let id = session.id;
    sessions.insert(session);
    let origin = ProxyOrigin::new("127.0.0.1", 1);
    let state = ProxyState {
        sessions,
        secret: AppSecret::from_bytes([7u8; 32]),
        client: upstream::build_client().expect("client builds"),
        origin: origin.clone(),
        host_budget: host_budget::HostBudget::fresh(),
    };
    (build_router(state), id, origin)
}

async fn get(router: &Router, uri: &str) -> (StatusCode, String) {
    let resp = router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(uri)
                .body(axum::body::Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// Every rendition a master names — a variant with an uppercase
/// extension, one with no extension and a query, and an audio
/// rendition with neither — comes back from the proxy as a rewritten
/// playlist whose segment points at the proxy.
#[tokio::test]
async fn every_rendition_a_master_names_comes_back_rewritten_whatever_its_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v/master.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "#EXTM3U\n\
             #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"jp\",URI=\"audio/list\"\n\
             #EXT-X-STREAM-INF:BANDWIDTH=2,RESOLUTION=1920x1080,AUDIO=\"a\"\n\
             1080/INDEX.M3U8\n\
             #EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1280x720,AUDIO=\"a\"\n\
             playlist?id=720\n",
        ))
        .mount(&server)
        .await;
    for rendition in ["/v/audio/list", "/v/1080/INDEX.M3U8", "/v/playlist"] {
        Mock::given(method("GET"))
            .and(wm_path(rendition))
            .respond_with(ResponseTemplate::new(200).set_body_string(MEDIA))
            .mount(&server)
            .await;
    }

    let (router, id, origin) = proxy_on(&format!("{}/v/master.m3u8", server.uri()));
    let (status, master) = get(&router, &format!("/s/{}/master.m3u8", id.as_string())).await;
    assert_eq!(status, StatusCode::OK);

    let mut renditions: Vec<String> = master
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(str::to_owned)
        .collect();
    renditions.extend(master.lines().filter_map(|l| {
        let uri = l.split("URI=\"").nth(1)?;
        Some(uri.split('"').next()?.to_owned())
    }));
    assert_eq!(renditions.len(), 3, "three renditions in {master}");

    for rendition in renditions {
        let path = rendition
            .strip_prefix(&origin.base)
            .unwrap_or_else(|| panic!("{rendition} is the proxy's"));
        let (status, body) = get(&router, path).await;
        assert_eq!(status, StatusCode::OK, "{rendition}");
        assert!(
            body.contains("/seg?u=") && !body.lines().any(|l| l == "seg-001.ts"),
            "{rendition} came back as a rewritten playlist: {body}"
        );
    }
}
