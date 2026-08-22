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
