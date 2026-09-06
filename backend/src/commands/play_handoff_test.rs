//! The two handoffs — external player and Syncplay — resolve the
//! same way the embedded player does.

use super::play::PlayArgs;
use crate::app::AppState;

fn state_for(td: &tempfile::TempDir, anidb_base: &str) -> AppState {
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::sync::Arc;
    AppState {
        anidb_base: Some(anidb_base.to_string()),
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
        cache_pool: crate::cache::open_in_memory().expect("in-mem cache pool"),
        kitsu: KitsuClient::with_base(reqwest::Client::new(), "http://127.0.0.1:1"),
        config_path: td.path().join("config.toml"),
        state_dir: std::path::PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// One show, one episode, a jpn embed and a validating master.
async fn stub_provider() -> wiremock::MockServer {
    use wiremock::matchers::{method, path};
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/browse"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(
                r#"<a href="/anime/handoff-show-7"><img alt="Handoff Show"/></a>"#,
            ),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/api/frontend/anime/7/episodes"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"episodes":[{"id":71,"number":1}]}"#),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/api/frontend/episode/71/languages"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(format!(
                r#"{{"languages":[{{"code":"jpn","embed_url":"{}/embed/71"}}]}}"#,
                server.uri()
            )),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/embed/71"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(format!(
                "player.setup({{ file: '{}/m/master.m3u8' }});",
                server.uri()
            )),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/m/master.m3u8"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("#EXTM3U\n"))
        .mount(&server)
        .await;
    server
}

fn args_for() -> PlayArgs {
    serde_json::from_value(serde_json::json!({
        "title": "Handoff Show",
        "episode": "1",
        "mode": "sub",
    }))
    .expect("args")
}

#[tokio::test]
async fn the_handoff_resolves_through_the_native_walk() {
    // Both handoffs used to shell out to the script for the stream URL.
    // The provider is anidb now, and the walk that serves the embedded
    // player serves these too — which is what this asserts: the state's
    // provider base is the stub below, and the launch args come back
    // carrying the URL that stub answered with.
    let server = stub_provider().await;
    let td = tempfile::tempdir().expect("td");
    let state = state_for(&td, &server.uri());
    let launch = super::play_handoff::resolve_launch_args(&state, &args_for())
        .await
        .expect("the native walk resolves the stream");
    assert!(
        launch.stream_url.ends_with("/m/master.m3u8"),
        "the handoff plays what the walk resolved: {}",
        launch.stream_url
    );
    assert_eq!(
        launch.referer, None,
        "anidb streams carry no referer requirement, as the embedded path already records"
    );
    assert_eq!(
        launch.title.as_deref(),
        Some("Handoff Show · ep 1"),
        "the player window keeps naming the Kitsu title and episode"
    );
}

#[tokio::test]
async fn a_handoff_miss_surfaces_the_walks_verdict() {
    // A clean no-results walk is the show being absent, not a
    // spawn failure — the caller renders it as such.
    use wiremock::matchers::method;
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"<div class="grid"><p>No results.</p></div>"#),
        )
        .mount(&server)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state_for(&td, &server.uri());
    let err = super::play_handoff::resolve_launch_args(&state, &args_for())
        .await
        .expect_err("nothing matches");
    assert!(matches!(err, crate::error::AniError::NoResults));
}

/// The handoff describes the launch from the resolve, referer
/// included — the external player and Syncplay both take one, and a
/// stream whose CDN checks it plays nowhere without it.
/// The players fetch their subtitles themselves, so the handoff hands
/// them the tracks' upstream URLs — the referer flag they already get
/// covers those fetches too.
#[test]
fn launch_args_carry_the_resolves_sidecar_tracks() {
    let cfg = crate::config::Config::default();
    let native = crate::commands::play_native_resolve::NativeResolved {
        slug: "the-show-77".into(),
        title: "The Show".into(),
        master_url: "https://cdn.example/x/master.m3u8".into(),
        episode_cap: Some(3),
        numbering_offset: 0,
        extra_tags: Vec::new(),
        resolved_slot: 1,
        resolved_tag: None,
        provider: crate::scraper::provider::ProviderId::Anidb,
        referer: Some("https://embed.example/".into()),
        subtitles: vec![crate::scraper::provider::SubtitleTrack {
            lang: "en".into(),
            label: "English".into(),
            default: true,
            url: "https://cdn.example/x/subs/en.vtt".into(),
        }],
    };
    let launch = super::play_handoff::launch_args_for(native, &args_for(), &cfg);
    assert_eq!(
        launch.subtitle_urls,
        vec!["https://cdn.example/x/subs/en.vtt".to_string()]
    );
}

#[test]
fn launch_args_carry_the_resolves_referer() {
    let cfg = crate::config::Config::default();
    let native = |referer: Option<&str>| crate::commands::play_native_resolve::NativeResolved {
        slug: "the-show-77".into(),
        title: "The Show".into(),
        master_url: "https://cdn.example/x/master.m3u8".into(),
        episode_cap: Some(3),
        numbering_offset: 0,
        extra_tags: Vec::new(),
        resolved_slot: 1,
        resolved_tag: None,
        provider: crate::scraper::provider::ProviderId::Anidb,
        referer: referer.map(str::to_string),
        subtitles: Vec::new(),
    };
    let with = super::play_handoff::launch_args_for(
        native(Some("https://embed.example/")),
        &args_for(),
        &cfg,
    );
    assert_eq!(with.stream_url, "https://cdn.example/x/master.m3u8");
    assert_eq!(with.referer.as_deref(), Some("https://embed.example/"));
    let without = super::play_handoff::launch_args_for(native(None), &args_for(), &cfg);
    assert_eq!(without.referer, None);
}

/// A player that takes one subtitle file takes the first listed, so
/// the track the provider flagged default leads the list; the
/// provider's order stands otherwise.
#[test]
fn launch_args_lead_with_the_providers_default_track() {
    use crate::scraper::provider::SubtitleTrack;
    let track = |lang: &str, default: bool| SubtitleTrack {
        lang: lang.into(),
        label: lang.into(),
        default,
        url: format!("https://cdn.example/x/subs/{lang}.vtt"),
    };
    let cfg = crate::config::Config::default();
    let native = crate::commands::play_native_resolve::NativeResolved {
        slug: "show-1".into(),
        provider: crate::scraper::provider::ProviderId::Anidb,
        title: "Show".into(),
        master_url: "https://cdn.example/x/master.m3u8".into(),
        episode_cap: None,
        numbering_offset: 0,
        extra_tags: vec![],
        resolved_slot: 1,
        resolved_tag: None,
        referer: None,
        subtitles: vec![track("ar", false), track("en", true), track("es", false)],
    };
    let launch = super::play_handoff::launch_args_for(native, &args_for(), &cfg);
    assert_eq!(
        launch.subtitle_urls,
        vec![
            "https://cdn.example/x/subs/en.vtt".to_string(),
            "https://cdn.example/x/subs/ar.vtt".to_string(),
            "https://cdn.example/x/subs/es.vtt".to_string(),
        ]
    );
}

mod default_first_props {
    use super::super::play_handoff::subtitle_urls_default_first;
    use crate::scraper::provider::SubtitleTrack;
    use proptest::prelude::*;

    /// Languages and default flags vary freely; URLs are distinct by
    /// position, as a listing never names one file twice and the
    /// order properties below tell tracks apart by URL.
    fn tracks() -> impl Strategy<Value = Vec<SubtitleTrack>> {
        prop::collection::vec(("[a-z]{2}", any::<bool>()), 0..6).prop_map(|rows| {
            rows.into_iter()
                .enumerate()
                .map(|(i, (lang, default))| SubtitleTrack {
                    label: lang.clone(),
                    url: format!("https://cdn.example/{i}.vtt"),
                    lang,
                    default,
                })
                .collect()
        })
    }

    proptest! {
        /// Every URL survives, exactly once, and a default track — when
        /// there is one — comes first.
        #[test]
        fn the_urls_are_a_permutation_led_by_a_default(tracks in tracks()) {
            let urls = subtitle_urls_default_first(&tracks);
            let mut expected: Vec<String> = tracks.iter().map(|t| t.url.clone()).collect();
            let mut got = urls.clone();
            expected.sort();
            got.sort();
            prop_assert_eq!(got, expected);
            if let Some(first) = urls.first() {
                let first_is_default = tracks.iter().any(|t| t.default && &t.url == first);
                let any_default = tracks.iter().any(|t| t.default);
                prop_assert!(first_is_default || !any_default);
            }
        }

        /// Relative order among the rest is the provider's.
        #[test]
        fn the_non_default_order_is_the_providers(tracks in tracks()) {
            let urls = subtitle_urls_default_first(&tracks);
            let rest: Vec<&String> = tracks.iter().filter(|t| !t.default).map(|t| &t.url).collect();
            let kept: Vec<&String> = urls.iter().filter(|u| rest.contains(u)).collect();
            prop_assert_eq!(kept, rest);
        }
    }
}

/// Sending an episode to a player is a watch, and the handoff writes
/// the history row for it; the watched-at stamp beside the row is
/// what orders Continue Watching and picks the row to resume from
/// when two providers each left one, so the handoff stamps it too.
#[tokio::test]
async fn the_handoff_stamps_the_watch_beside_its_history_row() {
    let server = stub_provider().await;
    let td = tempfile::tempdir().expect("td");
    let state = state_for(&td, &server.uri());
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    super::play_handoff::resolve_launch_args(&state, &args_for())
        .await
        .expect("the native walk resolves the stream");
    let stamp = crate::commands::kitsu::watched_at_get(&state, "handoff-show-7")
        .expect("stamp read")
        .expect("the handoff stamped its watch");
    assert!(stamp >= before, "stamped with the handoff's own moment");
}
