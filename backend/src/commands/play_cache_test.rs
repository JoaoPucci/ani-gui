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
        hianime_base: None,
        hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        provider_order: vec![crate::scraper::provider::ProviderId::Anidb],
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

/// The handoff's cache hit stands on the same row the embedded
/// player's does, and refreshes it the same way: the provider the
/// cached show key names is remembered again after the launch.
#[tokio::test]
async fn a_cached_handoff_refreshes_the_providers_positive_row() {
    let mock = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&mock)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state_in(&td);
    let args: crate::commands::play::PlayArgs = serde_json::from_value(
        serde_json::json!({ "title": "Show", "episode": "1", "mode": "sub", "kitsu_id": "K12" }),
    )
    .expect("args");
    let key = crate::commands::play_resolution_cache::cache_key(
        &args.title,
        &args.mode,
        "best",
        &args.episode,
        args.year,
        args.episode_count,
        args.subtype.as_deref(),
    );
    crate::commands::play_resolution_cache::put(
        &state.cache_pool,
        &key,
        &CachedResolution {
            upstream_url: format!("{}/cached/master.m3u8", mock.uri()),
            referer: String::new(),
            media_kind: MediaKind::Hls,
            show_id: "hianime:show-1".into(),
            show_title: "Show".into(),
            resolved_slot: Some(1),
            subtitles: Vec::new(),
        },
    );
    let cfg = crate::config::Config {
        cache_resolutions: true,
        ..Default::default()
    };
    let launched = super::try_launch_args_from_cache(&state, &args, &cfg).await;
    assert!(launched.is_some(), "the row is live and is served");
    assert_eq!(
        crate::commands::availability::cached_provider(&state, "K12", "sub"),
        Some(crate::scraper::provider::ProviderId::Hianime)
    );
}

mod row_provider_props {
    use super::super::cached_row_provider;
    use crate::commands::play_resolution_cache::CachedResolution;
    use crate::proxy::MediaKind;
    use crate::scraper::provider::{ProviderId, ShowKey};
    use proptest::prelude::*;

    fn row(show_id: &str) -> CachedResolution {
        CachedResolution {
            upstream_url: "https://cdn.example/x/master.m3u8".into(),
            referer: String::new(),
            media_kind: MediaKind::Hls,
            show_id: show_id.into(),
            show_title: "Show".into(),
            resolved_slot: None,
            subtitles: Vec::new(),
        }
    }

    proptest! {
        /// A row's provider is the one its show key names — the
        /// qualified prefix, or anidb for a bare key — and a row from
        /// before the field, with an empty key, names none.
        #[test]
        fn the_rows_provider_is_the_one_its_show_key_names(
            prefix in proptest::option::of(Just("hianime:")),
            slug in "[a-z0-9-]{1,20}",
        ) {
            let id = format!("{}{slug}", prefix.unwrap_or(""));
            prop_assert_eq!(cached_row_provider(&row(&id)), Some(ShowKey::parse(&id).provider));
            prop_assert_eq!(
                cached_row_provider(&row(&id)),
                Some(if prefix.is_some() { ProviderId::Hianime } else { ProviderId::Anidb })
            );
            prop_assert_eq!(cached_row_provider(&row("")), None);
        }
    }
}
