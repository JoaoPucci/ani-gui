//! The cache-hit handoff projects a cached row onto launch
//! arguments the same way a fresh resolve does.

use super::cached_launch_args;
use crate::commands::play_resolution_cache::CachedResolution;
use crate::proxy::MediaKind;
use crate::scraper::provider::SubtitleTrack;

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
