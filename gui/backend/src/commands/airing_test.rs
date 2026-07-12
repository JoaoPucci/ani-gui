//! Tests for `crate::commands::airing`. Extracted via `#[path]` per
//! `project_crap_inline_test_gotcha`.

use super::*;

/// AppState whose Kitsu client points at a wiremock server. Same
/// shape as `account_test::state_with_kitsu` (that helper is private
/// to its module).
fn state_with_kitsu(kitsu_uri: &str) -> std::sync::Arc<AppState> {
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    Arc::new(AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        ani_cli_path: PathBuf::from("/tmp/ani-cli"),
        bash_path: None,
        bundled_bin: None,
        history_path: PathBuf::from("/tmp/ani-cli/ani-hsts"),
        scraper_slots: Arc::new(Semaphore::new(crate::app::SCRAPER_CONCURRENCY)),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
    })
}

/// Yani Neko's real mapping shape: anilist/anime only.
const KITSU_ANILIST_ONLY_MAPPING_BODY: &str = r#"{
    "data": { "id": "50551", "type": "anime", "attributes": { "canonicalTitle": "Yani Neko" } },
    "included": [{
        "id": "1",
        "type": "mappings",
        "attributes": { "externalSite": "anilist/anime", "externalId": "207141" }
    }]
}"#;

const ANILIST_RELEASING_BODY: &str = r#"{"data":{"Media":{
    "status":"RELEASING","episodes":12,
    "nextAiringEpisode":{"episode":3,"airingAt":1784215800}
}}}"#;

#[tokio::test]
async fn airing_get_bridges_kitsu_mappings_to_anilist() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/50551"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_ANILIST_ONLY_MAPPING_BODY),
        )
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(ANILIST_RELEASING_BODY))
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = airing_get_with_anilist_base(&state, "50551", Some(&anilist.uri()))
        .await
        .expect("ok");
    assert_eq!(got.aired, Some(2));
    assert_eq!(got.next_episode, Some(3));
    assert_eq!(got.next_airing_at, Some(1_784_215_800));
}

#[tokio::test]
async fn airing_get_defaults_when_kitsu_has_no_mappings() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/777"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"id":"777","type":"anime"},"included":[]}"#),
        )
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("{}"))
        .expect(0)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = airing_get_with_anilist_base(&state, "777", Some(&anilist.uri()))
        .await
        .expect("ok");
    assert_eq!(got, AiringStatus::default());
}

#[tokio::test]
async fn airing_get_caches_per_show() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/50551"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_ANILIST_ONLY_MAPPING_BODY),
        )
        .expect(1)
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(ANILIST_RELEASING_BODY))
        .expect(1)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let first = airing_get_with_anilist_base(&state, "50551", Some(&anilist.uri()))
        .await
        .expect("ok");
    let second = airing_get_with_anilist_base(&state, "50551", Some(&anilist.uri()))
        .await
        .expect("ok");
    assert_eq!(first, second);
    assert_eq!(second.aired, Some(2));
    // Both MockServers verify their .expect(1) on drop — the second
    // call must be served from the cache.
}
