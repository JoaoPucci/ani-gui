//! The subtitle route: a session's sidecar track, fetched upstream
//! with the session's referer and served as WebVTT.

use super::*;
use crate::scraper::provider::SubtitleTrack;
use tower::ServiceExt as _;
use wiremock::matchers::{header, method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn proxy_with_tracks(referer: &str, tracks: Vec<SubtitleTrack>) -> (Router, String) {
    let sessions = SessionTable::new();
    let mut session = StreamSession::new(
        url::Url::parse("https://cdn.example/master.m3u8").expect("master url"),
        referer.to_string(),
    );
    session.subtitles = tracks;
    let id = session.id;
    sessions.insert(session);
    let state = ProxyState {
        sessions,
        secret: AppSecret::from_bytes([7u8; 32]),
        client: reqwest::Client::new(),
        origin: ProxyOrigin::new("127.0.0.1", 1),
    };
    (build_router(state), id.as_string())
}

#[tokio::test]
async fn a_sidecar_track_is_served_with_the_sessions_referer() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/subs/en.vtt"))
        .and(header("referer", "https://embed.example/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("WEBVTT\n\n00:00.000 --> 00:01.000\nhi\n")
                .insert_header("content-type", "text/vtt"),
        )
        .mount(&server)
        .await;
    let (router, id) = proxy_with_tracks(
        "https://embed.example/",
        vec![SubtitleTrack {
            lang: "en".into(),
            label: "English".into(),
            default: true,
            url: format!("{}/subs/en.vtt", server.uri()),
        }],
    )
    .await;
    let resp = router
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/s/{id}/sub/0.vtt"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/vtt")
    );
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .expect("body");
    assert!(
        body.starts_with(b"WEBVTT"),
        "the track body passes through verbatim"
    );
}

#[tokio::test]
async fn an_unknown_track_index_is_not_found() {
    let (router, id) = proxy_with_tracks("https://embed.example/", Vec::new()).await;
    let resp = router
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/s/{id}/sub/0.vtt"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
