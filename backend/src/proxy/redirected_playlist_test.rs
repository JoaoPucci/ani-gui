//! Which URL a playlist's relative URIs are resolved against once the
//! upstream has redirected.
//!
//! Extracted via `#[path]` so the inline `#[cfg(test)]` module's
//! complexity doesn't pile onto `mod.rs`'s CCN budget — per
//! `project_crap_inline_test_gotcha`.
//!
//! The transport follows redirects, so the manifest the proxy holds
//! came from wherever the upstream sent it, and a manifest's relative
//! URIs are relative to that place. A CDN that moves a stream to
//! another directory or host answers the old URL with a redirect and
//! a manifest naming `720/index.m3u8` beside the new one; resolved
//! against the URL the session stored, that names a rendition beside
//! the old one, which the CDN has nothing at.

use super::*;
use tower::ServiceExt as _;
use wiremock::matchers::{method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A proxy fronting `server`, holding one HLS session on `master`.
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

/// The upstream URLs a rewritten manifest's proxied lines point at,
/// decoded out of their `u=` parameter.
fn upstreams_named_by(rewritten: &str) -> Vec<String> {
    rewritten
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let u = l.split("u=").nth(1)?.split('&').next()?;
            let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(u)
                .ok()?;
            String::from_utf8(bytes).ok()
        })
        .collect()
}

async fn body_of(resp: Response) -> String {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    String::from_utf8(bytes.to_vec()).expect("utf8")
}

/// The master the session stored redirects to another directory,
/// and the manifest there names its rendition relatively: the
/// rendition the proxy points the player at sits beside the manifest
/// that named it, not beside the URL the session stored.
#[tokio::test]
async fn a_redirected_masters_relative_rendition_resolves_against_where_it_landed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/old/master.m3u8"))
        .respond_with(ResponseTemplate::new(302).insert_header(
            "location",
            format!("{}/new/master.m3u8", server.uri()).as_str(),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(wm_path("/new/master.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1280x720\n720/index.m3u8\n",
        ))
        .mount(&server)
        .await;

    let (router, id, _secret) = proxy_on(&format!("{}/old/master.m3u8", server.uri()));
    let resp = router
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/s/{}/master.m3u8", id.as_string()))
                .body(axum::body::Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        upstreams_named_by(&body_of(resp).await),
        vec![format!("{}/new/720/index.m3u8", server.uri())],
        "the rendition sits beside the manifest that named it"
    );
}

/// The same for a media playlist reached through the segment route:
/// a rendition that redirects names its segments relative to where
/// it landed.
#[tokio::test]
async fn a_redirected_media_playlists_relative_segments_resolve_against_where_it_landed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/old/720/index.m3u8"))
        .respond_with(ResponseTemplate::new(302).insert_header(
            "location",
            format!("{}/new/720/index.m3u8", server.uri()).as_str(),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(wm_path("/new/720/index.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\nseg-001.ts\n#EXT-X-ENDLIST\n",
        ))
        .mount(&server)
        .await;

    let master = format!("{}/old/master.m3u8", server.uri());
    let (router, id, secret) = proxy_on(&master);
    let rendition = format!("{}/old/720/index.m3u8", server.uri());
    let token = sign_segment(&secret, id, &rendition);
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rendition.as_bytes());
    let resp = router
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/s/{}/seg?u={encoded}&t={token}", id.as_string()))
                .body(axum::body::Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        upstreams_named_by(&body_of(resp).await),
        vec![format!("{}/new/720/seg-001.ts", server.uri())],
        "the segment sits beside the playlist that named it"
    );
}
