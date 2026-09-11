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

fn state_in(td: &tempfile::TempDir) -> crate::app::AppState {
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::sync::Arc;
    crate::app::AppState {
        anidb_base: Some("http://127.0.0.1:1".into()),
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: td.path().join("history"),
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        image_cache_dir: td.path().join("images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::with_base(reqwest::Client::new(), "http://127.0.0.1:1"),
        config_path: td.path().join("config.toml"),
        state_dir: td.path().join("state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// The history file speaks the provider's numbering, and a cached
/// row carries the slot its resolve landed on. A replay records that
/// slot — the display number translated through the single display
/// stamp can point at a recap once a later resolve has moved the
/// stamp — and only a row from before the field falls back to the
/// translation.
#[test]
fn a_cached_watch_prefers_the_rows_own_slot() {
    let td = tempfile::tempdir().expect("td");
    let state = state_in(&td);
    let row = |slot: Option<u32>| CachedResolution {
        upstream_url: "https://cdn.example/x/master.m3u8".into(),
        referer: String::new(),
        media_kind: MediaKind::Hls,
        show_id: "show-1".into(),
        show_title: "Show".into(),
        resolved_slot: slot,
        subtitles: Vec::new(),
    };
    let stamped = super::cached_watch(&state, &row(Some(5)), "4");
    assert_eq!(
        stamped.ep_no, "5",
        "the row's own slot, not the display number"
    );
    assert_eq!(stamped.show_id, "show-1");
    assert_eq!(stamped.title, "Show");
    let legacy = super::cached_watch(&state, &row(None), "4");
    assert_eq!(
        legacy.ep_no, "4",
        "a row without a slot translates the display number"
    );
}
