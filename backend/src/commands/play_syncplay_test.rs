use super::*;
use crate::commands::play::PlayArgs;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Same one-show provider surface as the play_external tests: browse,
/// episodes, jpn embed on episode 2, master URL on the embed page.
async fn stub_provider(mock: &MockServer, query: &str) {
    Mock::given(method("GET"))
        .and(path("/browse"))
        .and(query_param("q", query))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("<a href=\"/anime/the-show-77\"><img alt=\"The Show\"/></a>"),
        )
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/frontend/anime/77/episodes"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"episodes":[{"id":701,"number":1},{"id":702,"number":2}]}"#),
        )
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/frontend/episode/702/languages"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!(
            r#"{{"languages":[{{"code":"jpn","embed_url":"{}/e/x"}}]}}"#,
            mock.uri()
        )))
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/e/x"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!(
            "player.setup({{ file: '{}/x/master.m3u8' }});",
            mock.uri()
        )))
        .mount(mock)
        .await;
    // The quality step validates the master before handing it on, so
    // the stub has to serve it like the CDN would.
    Mock::given(method("GET"))
        .and(path("/x/master.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string("#EXTM3U\n"))
        .mount(mock)
        .await;
}

fn stage_recorder(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let argv_file = dir.join("syncplay-argv");
    let binary = dir.join("recorder");
    std::fs::write(
        &binary,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"{}\"\n",
            argv_file.display()
        ),
    )
    .expect("write recorder");
    let mut perms = std::fs::metadata(&binary).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&binary, perms).expect("chmod");
    (binary, argv_file)
}

fn state_for(dir: &std::path::Path, provider_base: &str) -> crate::app::AppState {
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::sync::Arc;
    crate::app::AppState {
        anidb_base: Some(provider_base.to_string()),
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: dir.join("history"),
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        image_cache_dir: dir.join("images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::new(reqwest::Client::new()),
        config_path: dir.join("config.toml"),
        state_dir: dir.join("state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

fn play_args() -> PlayArgs {
    PlayArgs {
        title: "the show".into(),
        episode: "2".into(),
        mode: "sub".into(),
        quality: None,
        subtype: None,
        episode_count: Some(2),
        year: None,
        alt_titles: Vec::new(),
        prefetch: false,
        kitsu_id: None,
    }
}

#[tokio::test]
async fn a_fresh_syncplay_launch_resolves_natively_and_receives_the_master_url() {
    // Empty cache forces the fresh path; the provider is unreachable,
    // so success proves the resolution never left the native client.
    let mock = MockServer::start().await;
    stub_provider(&mock, "the show").await;
    let dir = tempfile::tempdir().expect("tmp");
    let (binary, argv_file) = stage_recorder(dir.path());
    let state = state_for(dir.path(), &mock.uri());
    std::fs::write(
        &state.config_path,
        format!("syncplay_binary = \"{}\"\n", binary.display()),
    )
    .expect("write config");

    play_syncplay(&state, &play_args()).await.expect("launches");

    let mut argv = String::new();
    for _ in 0..100 {
        if let Ok(s) = std::fs::read_to_string(&argv_file) {
            if !s.is_empty() {
                argv = s;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(
        argv.contains(&format!("{}/x/master.m3u8", mock.uri())),
        "syncplay must receive the master playlist URL; got: {argv}"
    );
}

/// Syncplay's cache-hit launch records the watch once the spawn
/// succeeded, exactly as the external player's does.
#[tokio::test]
async fn a_cached_syncplay_launch_records_the_watch_after_the_spawn() {
    let mock = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/cached/master.m3u8"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;
    let dir = tempfile::tempdir().expect("tmp");
    let (binary, argv_file) = stage_recorder(dir.path());
    let state = state_for(dir.path(), "http://127.0.0.1:1");
    std::fs::write(
        &state.config_path,
        format!(
            "syncplay_binary = \"{}\"\ncache_resolutions = true\n",
            binary.display()
        ),
    )
    .expect("write config");
    let args = play_args();
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
        &crate::commands::play_resolution_cache::CachedResolution {
            upstream_url: format!("{}/cached/master.m3u8", mock.uri()),
            referer: String::new(),
            media_kind: crate::proxy::MediaKind::Hls,
            show_id: "cached-show-9".into(),
            show_title: "Cached Show".into(),
            resolved_slot: Some(2),
            subtitles: Vec::new(),
        },
    );

    play_syncplay(&state, &args)
        .await
        .expect("launches from the cache");

    let mut argv = String::new();
    for _ in 0..100 {
        if let Ok(s) = std::fs::read_to_string(&argv_file) {
            if !s.is_empty() {
                argv = s;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(
        argv.contains("/cached/master.m3u8"),
        "the cached URL reached Syncplay: {argv}"
    );
    let hsts = std::fs::read_to_string(&state.history_path).expect("history written");
    assert!(hsts.contains("cached-show-9"), "{hsts}");
    assert!(
        crate::commands::kitsu::watched_at_get(&state, "cached-show-9")
            .expect("stamp read")
            .is_some(),
        "the watch is stamped once Syncplay started"
    );
}
