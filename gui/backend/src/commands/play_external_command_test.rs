use super::*;
use crate::commands::play::PlayArgs;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Mount the full native resolution surface for one show on `mock`:
/// browse for `query`, a two-episode list, a jpn embed on episode 2,
/// and the embed page carrying the master playlist URL.
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

/// Mount a listing whose provider slots and display tags diverge:
/// slot 4 is the recap tagged `3.5`, so regular episode 4 lives in
/// slot 5. Requesting display episode 4 must resolve slot 5, and the
/// history row has to carry the slot — the script's reader greps the
/// stored number against the provider's own listing, where 4 is the
/// recap.
async fn stub_provider_with_a_recap(mock: &MockServer, query: &str) {
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
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"episodes":[
                {"id":701,"number":1,"number2":null},
                {"id":702,"number":2,"number2":null},
                {"id":703,"number":3,"number2":null},
                {"id":704,"number":4,"number2":3.5},
                {"id":705,"number":5,"number2":4}
            ]}"#,
        ))
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/frontend/episode/705/languages"))
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
    Mock::given(method("GET"))
        .and(path("/x/master.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string("#EXTM3U\n"))
        .mount(mock)
        .await;
}

/// A player binary that records its argv and exits — the observable
/// end of the spawn-and-detach launch.
fn stage_recorder(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let argv_file = dir.join("player-argv");
    let player = dir.join("recorder");
    std::fs::write(
        &player,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"{}\"\n",
            argv_file.display()
        ),
    )
    .expect("write recorder");
    let mut perms = std::fs::metadata(&player).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&player, perms).expect("chmod");
    (player, argv_file)
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

async fn wait_for(argv_file: &std::path::Path) -> String {
    for _ in 0..100 {
        if let Ok(s) = std::fs::read_to_string(argv_file) {
            if !s.is_empty() {
                return s;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("player was never launched (no argv recorded)");
}

#[tokio::test]
async fn a_fresh_external_play_resolves_natively_and_hands_the_player_the_master_url() {
    // Cache is empty, so the fresh path runs. Everything the walk
    // needs is stubbed on the provider origin; the provider base points at
    // a nonexistent binary, so an Ok can only come from the native
    // resolution — a subprocess attempt would fail the play.
    let mock = MockServer::start().await;
    stub_provider(&mock, "the show").await;
    let dir = tempfile::tempdir().expect("tmp");
    let (player, argv_file) = stage_recorder(dir.path());
    let state = state_for(dir.path(), &mock.uri());
    std::fs::write(
        &state.config_path,
        format!("external_player = \"{}\"\n", player.display()),
    )
    .expect("write config");

    play_external(&state, &play_args()).await.expect("plays");

    let argv = wait_for(&argv_file).await;
    assert!(
        argv.contains(&format!("{}/x/master.m3u8", mock.uri())),
        "player must receive the master playlist URL; got: {argv}"
    );
    let browsed = mock
        .received_requests()
        .await
        .expect("recorded")
        .iter()
        .any(|r| r.url.path() == "/browse");
    assert!(browsed, "the native walk must have searched the provider");
}

#[tokio::test]
async fn an_external_play_lands_in_history_under_the_provider_slug() {
    // The subprocess used to write the history file itself; the native path
    // owns that write now, keyed on the slug like embedded play.
    let mock = MockServer::start().await;
    stub_provider(&mock, "the show").await;
    let dir = tempfile::tempdir().expect("tmp");
    let (player, argv_file) = stage_recorder(dir.path());
    let state = state_for(dir.path(), &mock.uri());
    std::fs::write(
        &state.config_path,
        format!("external_player = \"{}\"\n", player.display()),
    )
    .expect("write config");

    play_external(&state, &play_args()).await.expect("plays");
    wait_for(&argv_file).await;

    let hsts = std::fs::read_to_string(&state.history_path).expect("history written");
    assert!(
        hsts.contains("the-show-77"),
        "history row must key on the provider slug; got: {hsts}"
    );
}

#[tokio::test]
async fn an_external_play_records_the_provider_slot_not_the_display_number() {
    // The history file speaks the provider's numbering: the script's
    // reader greps the stored number against the show's own listing.
    // On a show with a recap, display episode 4 is slot 5 — storing
    // the display number points the row at the recap instead, and
    // the rail then offers to resume "3.5".
    let mock = MockServer::start().await;
    stub_provider_with_a_recap(&mock, "the show").await;
    let dir = tempfile::tempdir().expect("tmp");
    let (player, argv_file) = stage_recorder(dir.path());
    let state = state_for(dir.path(), &mock.uri());
    std::fs::write(
        &state.config_path,
        format!("external_player = \"{}\"\n", player.display()),
    )
    .expect("write config");

    let mut args = play_args();
    args.episode = "4".into();
    args.episode_count = Some(4);
    play_external(&state, &args).await.expect("plays");
    wait_for(&argv_file).await;

    let hsts = std::fs::read_to_string(&state.history_path).expect("history written");
    let ep_no = hsts.split_whitespace().next().expect("a history row");
    assert_eq!(
        ep_no, "5",
        "the row must carry the resolved provider slot, not the display tag: {hsts}"
    );
}
