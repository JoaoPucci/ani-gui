//! The cache-hit handoff projects a cached row onto launch
//! arguments the same way a fresh resolve does, and a cached row is
//! live only when its tracks read as tracks.

use super::{cached_launch_args, try_serve_cached, webvtt_prefix};
use crate::commands::play::tests::{cached_blank, state_with_proxy_origin, track};
use crate::commands::play_resolution_cache::CachedResolution;
use crate::proxy::MediaKind;
use crate::scraper::provider::SubtitleTrack;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A player that takes one subtitle file takes the first listed, so
/// the cached row's default track leads here as it does on a fresh
/// resolve.
#[test]
fn cached_launch_args_lead_with_the_providers_default_track() {
    let track = |lang: &str, default: bool| SubtitleTrack {
        lang: lang.into(),
        label: lang.into(),
        default,
        url: format!("https://cdn.example/x/subs/{lang}.vtt"),
    };
    let cached = CachedResolution {
        upstream_url: "https://cdn.example/x/master.m3u8".into(),
        referer: String::new(),
        media_kind: MediaKind::Hls,
        show_id: "show-1".into(),
        show_title: "Show".into(),
        resolved_slot: Some(1),
        subtitles: vec![track("ar", false), track("en", true), track("es", false)],
    };
    let args: crate::commands::play::PlayArgs = serde_json::from_value(
        serde_json::json!({ "title": "Show", "episode": "1", "mode": "sub" }),
    )
    .expect("args");
    let cfg = crate::config::Config::default();
    let launch = cached_launch_args(cached, &args, &cfg);
    assert_eq!(
        launch.subtitle_urls,
        vec![
            "https://cdn.example/x/subs/en.vtt".to_string(),
            "https://cdn.example/x/subs/ar.vtt".to_string(),
            "https://cdn.example/x/subs/es.vtt".to_string(),
        ]
    );
    assert_eq!(launch.stream_url, "https://cdn.example/x/master.m3u8");
    assert_eq!(launch.referer, None, "an empty cached referer is none");
}

// ── a cached track is read the way the relay reads it ──────────────

/// A stream that pings, a track that answers as the given GET does.
async fn row_with_track(server: &MockServer, track_response: ResponseTemplate) -> CachedResolution {
    Mock::given(method("HEAD"))
        .and(path("/video.mp4"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/subs/en.vtt"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/subs/en.vtt"))
        .respond_with(track_response)
        .mount(server)
        .await;
    let mut cached = cached_blank(
        format!("{}/video.mp4", server.uri()),
        String::new(),
        MediaKind::Mp4,
    );
    cached.subtitles = vec![track("en", true, &format!("{}/subs/en.vtt", server.uri()))];
    cached
}

/// A CDN can answer a HEAD with 200 and a GET with a challenge page,
/// and the relay refuses the page as not a track — so a row proved
/// live by HEAD alone replayed without subtitles, on every replay,
/// for the rest of its life, since a track that fails to load never
/// reaches the player's recovery path. A track is live only when a
/// GET with the row's referer answers as a track.
#[tokio::test]
async fn a_cached_track_whose_get_is_not_a_track_is_a_dead_row() {
    let server = MockServer::start().await;
    let cached = row_with_track(
        &server,
        ResponseTemplate::new(200).set_body_string("<html><title>Just a moment</title></html>"),
    )
    .await;
    let state = state_with_proxy_origin();
    assert!(
        try_serve_cached(&state, &cached).await.is_none(),
        "a track that does not read as a track is a dead row"
    );
    let gets: Vec<String> = server
        .received_requests()
        .await
        .expect("recorded")
        .iter()
        .filter(|r| r.method == "GET")
        .map(|r| r.url.path().to_string())
        .collect();
    assert_eq!(gets, vec!["/subs/en.vtt".to_string()], "the track was read");
}

/// The control: a track that answers a GET as a track keeps the row
/// live, behind a byte-order mark as the relay allows.
#[tokio::test]
async fn a_cached_track_whose_get_is_a_track_keeps_the_row_live() {
    let server = MockServer::start().await;
    let cached = row_with_track(
        &server,
        ResponseTemplate::new(200)
            .set_body_bytes(b"\xEF\xBB\xBFWEBVTT\n\n00:00.000 --> 00:01.000\nhi\n".to_vec()),
    )
    .await;
    let state = state_with_proxy_origin();
    assert!(
        try_serve_cached(&state, &cached).await.is_some(),
        "a track that reads as a track is live"
    );
}

mod webvtt_prefix_props {
    use super::webvtt_prefix;
    use crate::proxy::is_webvtt;
    use proptest::prelude::*;

    /// Bodies whose first bytes are close to the signature — a
    /// byte-order mark or not, then a prefix of `WEBVTT` or something
    /// else — followed by anything.
    fn body() -> impl Strategy<Value = Vec<u8>> {
        (
            proptest::option::of(Just(b"\xEF\xBB\xBF".to_vec())),
            prop_oneof![
                (0..=6usize).prop_map(|n| b"WEBVTT"[..n].to_vec()),
                proptest::collection::vec(any::<u8>(), 0..8),
            ],
            proptest::collection::vec(any::<u8>(), 0..16),
        )
            .prop_map(|(bom, head, rest)| {
                let mut b = bom.unwrap_or_default();
                b.extend(head);
                b.extend(rest);
                b
            })
    }

    proptest! {
        /// Reading a body from its first byte, the prefix verdict
        /// never contradicts the whole body's, and once the whole
        /// body is in hand it is never undecided unless the body is
        /// itself too short to be anything.
        #[test]
        fn a_decided_prefix_agrees_with_the_whole_body(body in body()) {
            let whole = is_webvtt(&body);
            for n in 0..=body.len() {
                if let Some(verdict) = webvtt_prefix(&body[..n]) {
                    prop_assert_eq!(verdict, whole, "prefix of {} bytes of {:?}", n, body);
                }
            }
            if body.len() >= 9 {
                prop_assert_eq!(webvtt_prefix(&body), Some(whole));
            }
        }
    }
}
