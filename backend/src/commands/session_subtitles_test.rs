//! Sidecar subtitle tracks ride the session: stored with it, and
//! offered to the renderer as proxied URLs it can hand a `<track>`.

use super::*;
use crate::proxy::SessionSubtitle;
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use crate::scraper::provider::SubtitleTrack;
use std::path::PathBuf;
use std::sync::Arc;

fn state() -> AppState {
    AppState {
        anidb_base: None,
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 4242),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/y/history"),
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

#[test]
fn a_session_offers_its_sidecar_tracks_through_the_proxy() {
    let state = state();
    let args = CreateSessionArgs {
        upstream_url: "https://cdn.example/x/master.m3u8".into(),
        referer: "https://embed.example/".into(),
        subtitles: vec![
            SubtitleTrack {
                lang: "en".into(),
                label: "English".into(),
                default: true,
                url: "https://cdn.example/x/subs/en.vtt".into(),
            },
            SubtitleTrack {
                lang: "es".into(),
                label: "Español".into(),
                default: false,
                url: "https://cdn.example/x/subs/es.vtt".into(),
            },
        ],
    };
    let resp = create_session_with_kind(&state, &args, MediaKind::Hls).expect("session");
    let base = format!("http://127.0.0.1:4242/s/{}", resp.session_id);
    assert_eq!(
        resp.subtitles,
        vec![
            SessionSubtitle {
                lang: "en".into(),
                label: "English".into(),
                default: true,
                url: format!("{base}/sub/0.vtt"),
            },
            SessionSubtitle {
                lang: "es".into(),
                label: "Español".into(),
                default: false,
                url: format!("{base}/sub/1.vtt"),
            },
        ],
        "the renderer never sees an upstream URL; every track is served by the proxy"
    );
    let stored = state
        .sessions
        .get(&crate::proxy::SessionId::parse(&resp.session_id).expect("id"))
        .expect("stored");
    assert_eq!(stored.subtitles, args.subtitles);
}

#[test]
fn a_session_without_tracks_offers_none() {
    let state = state();
    let args = CreateSessionArgs {
        upstream_url: "https://cdn.example/x/master.m3u8".into(),
        referer: String::new(),
        subtitles: Vec::new(),
    };
    let resp = create_session_with_kind(&state, &args, MediaKind::Hls).expect("session");
    assert!(resp.subtitles.is_empty());
}
