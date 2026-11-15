//! A play's session, and the playlist the proxy serves for it, name
//! the stream proxy's origin — on every platform.
//!
//! `lanes.rs` runs the real binary on every platform: it checks that
//! the stream proxy's origin is the stream lane, and that a play
//! through the binary names that lane. This is the same invariant in
//! process, against a state of the test's own: the play route answers
//! with a session on the proxy's origin, and the proxy's rewritten
//! playlist names that origin too — a failure here points at the
//! route or the proxy rather than at the binary's wiring.
//!
//! The play is a replay of a cache row the test writes, as it is in the
//! end-to-end specs: a resolve would walk a provider, and the walked one
//! here is pointed at a port nothing listens on, so a play that missed
//! the row would fail rather than reach the network.

use std::sync::Arc;

use ani_gui::app::AppState;
use ani_gui::cache;
use ani_gui::commands::play_resolution_cache::{cache_key, put, CachedResolution};
use ani_gui::meta::kitsu::KitsuClient;
use ani_gui::provider_set::ProviderSet;
use ani_gui::proxy::{AppSecret, MediaKind, ProxyOrigin, SessionTable};
use ani_gui::scraper::provider::ProviderId;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The port the state's stream proxy is said to listen on. Nothing
/// needs to: the proxy is driven in-process.
const STREAM_PORT: u16 = 47_101;

fn state(tmp: &std::path::Path) -> AppState {
    AppState {
        providers: ProviderSet::listing(vec![ProviderId::Hianime])
            .with_origin(ProviderId::Hianime, "http://127.0.0.1:1"),
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: ani_gui::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", STREAM_PORT),
        bundled_bin: None,
        legacy_sweep: ani_gui::legacy_script::SweepReport::default(),
        history_path: tmp.join("hist/history"),
        image_cache_dir: tmp.join("images"),
        cache_pool: cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::with_base(reqwest::Client::new(), "http://127.0.0.1:1"),
        config_path: tmp.join("config.toml"),
        state_dir: tmp.join("state"),
        internal_secret: ani_gui::account::InternalSecret::random(),
        mal_refresh: ani_gui::meta::mal_user::MalRefreshState::new(),
        account_write_locks: ani_gui::commands::account::AccountWriteLocks::new(),
        availability_refreshes: ani_gui::commands::availability_refresh::AvailabilityRefreshes::new(
        ),
        flags: ani_gui::flags::FlagStore::fixed(ani_gui::flags::DECLARED),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_plays_session_and_its_playlist_name_the_stream_proxys_origin() {
    let upstream = MockServer::start().await;
    for verb in ["GET", "HEAD"] {
        Mock::given(method(verb))
            .and(path("/master.m3u8"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000,RESOLUTION=640x360\nlow/index.m3u8\n",
            ))
            .mount(&upstream)
            .await;
    }

    let tmp = TempDir::new().expect("tempdir");
    std::fs::write(tmp.path().join("config.toml"), "cache_resolutions = true\n").expect("config");
    let state = Arc::new(state(tmp.path()));
    put(
        &state.cache_pool,
        &cache_key("Origin Test Show", "sub", "1", None, None, None),
        &CachedResolution {
            upstream_url: format!("{}/master.m3u8", upstream.uri()),
            referer: String::new(),
            media_kind: MediaKind::Hls,
            show_id: "hianime:origin-test-show-1".into(),
            show_title: "Origin Test Show".into(),
            resolved_slot: None,
            played: Some("1".into()),
            former_episode: None,
            subtitles: Vec::new(),
            kitsu_id: None,
            versions: Vec::new(),
            version: None,
            episode_unverified: false,
        },
    );

    let response = ani_gui::api::build_api_router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/play")
                .header("content-type", "application/json")
                .header(
                    ani_gui::account::INTERNAL_SECRET_HEADER,
                    state.internal_secret.as_hex(),
                )
                .body(Body::from(
                    r#"{"title":"Origin Test Show","mode":"sub","episode":"1"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("play");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let session: serde_json::Value = serde_json::from_slice(&body).expect("play json");
    assert_eq!(
        session["cache_hit"],
        serde_json::json!(true),
        "the play was the replay of the row: {session}"
    );

    let origin = format!("http://127.0.0.1:{STREAM_PORT}");
    let media_url = session["media_url"].as_str().unwrap_or_default();
    assert!(
        media_url.starts_with(&format!("{origin}/s/")),
        "the player's URL is on the stream proxy's origin ({origin}): {media_url}"
    );

    let master = ani_gui::proxy::build_router(state.proxy_state())
        .oneshot(
            Request::builder()
                .uri(&media_url[origin.len()..])
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("master");
    assert_eq!(master.status(), StatusCode::OK);
    let master = master.into_body().collect().await.expect("body").to_bytes();
    let master = String::from_utf8_lossy(&master);
    let variant = master
        .lines()
        .find(|l| !l.starts_with('#') && !l.is_empty())
        .unwrap_or_default();
    assert!(
        variant.starts_with(&format!("{origin}/s/")),
        "the rewritten playlist names the stream proxy's origin ({origin}): {master}"
    );
}
