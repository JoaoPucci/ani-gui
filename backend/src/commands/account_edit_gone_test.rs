//! The editor's commands for a show whose tracker entry the tracker no
//! longer has. Kitsu can map a show to an id the tracker has since
//! removed: MyAnimeList then answers 404 for `/anime/{id}` itself, and
//! AniList answers `Media(id:)` with its 404 `Not Found.` envelope. That
//! is an answer about the show, not the tracker failing: the read
//! answers as for a show no tracker id maps (`None`), and the writes
//! answer as the unmappable paths do — a save `None`, a removal `false`
//! — so nothing reports a write that did not happen.

use std::sync::Arc;

use super::{get_entry_via, remove_entry_via, set_entry_via};
use crate::account::provider::{EntryUpdate, ProviderKind, Tokens};
use crate::app::AppState;
use crate::meta::anilist_user::AniListProvider;
use crate::meta::mal_user::{MalProvider, MalRefreshState};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn state_with_kitsu(kitsu_uri: &str) -> crate::scratch::Scratch<Arc<AppState>> {
    let dirs = crate::scratch::dir();
    let state = Arc::new(AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: dirs.history(),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: dirs.images(),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: dirs.config(),
        state_dir: dirs.state(),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
        flags: crate::flags::FlagStore::fixed(crate::flags::DECLARED),
    });
    dirs.hold(state)
}

fn tokens() -> Tokens {
    Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

/// Kitsu 13023 ("Delete") as Kitsu serves it: mapped to MAL 33778,
/// which MyAnimeList no longer has.
const KITSU_MAL: &str = r#"{
    "data": { "id": "13023", "type": "anime", "attributes": { "canonicalTitle": "Delete" } },
    "included": [
        { "id": "1", "type": "mappings",
          "attributes": { "externalSite": "myanimelist/anime", "externalId": "33778" } }
    ]
}"#;

/// The same show mapped to AniList alone (an AniList account resolves a
/// MAL mapping through AniList's public API first, which these cases do
/// not stand up).
const KITSU_ANILIST: &str = r#"{
    "data": { "id": "13023", "type": "anime", "attributes": { "canonicalTitle": "Delete" } },
    "included": [
        { "id": "2", "type": "mappings",
          "attributes": { "externalSite": "anilist/anime", "externalId": "33778" } }
    ]
}"#;

async fn kitsu(body: &'static str) -> MockServer {
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/13023"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&kitsu)
        .await;
    kitsu
}

/// MyAnimeList without anime 33778: every request about it answers
/// 404, as `https://myanimelist.net/anime/33778` does; the bearer's
/// user is known.
async fn mal_without_the_anime() -> MockServer {
    let mal = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/@me"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id":4242,"name":"shiro"}"#))
        .mount(&mal)
        .await;
    Mock::given(path("/anime/33778"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mal)
        .await;
    Mock::given(path("/anime/33778/my_list_status"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mal)
        .await;
    mal
}

fn mal(server: &MockServer) -> MalProvider {
    MalProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_string(),
        MalRefreshState::new(),
    )
}

/// AniList without media 33778: every query about it answers AniList's
/// 404 envelope; the viewer is known.
async fn anilist_without_the_media() -> MockServer {
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains("Viewer"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Viewer":{"id":4242,"name":"shiro","avatar":{"large":null},"statistics":{"anime":{"count":1,"meanScore":0}}}}}"#,
        ))
        .mount(&anilist)
        .await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_string(crate::meta::anilist_media::ANILIST_NOT_FOUND_BODY),
        )
        .mount(&anilist)
        .await;
    anilist
}

fn anilist(server: &MockServer) -> AniListProvider {
    AniListProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_string(),
    )
}

fn watched(ep: u32) -> EntryUpdate {
    EntryUpdate {
        progress_episodes: Some(ep),
        ..Default::default()
    }
}

#[tokio::test]
async fn mal_reading_an_anime_mal_no_longer_has_answers_no_entry() {
    let kitsu = kitsu(KITSU_MAL).await;
    let server = mal_without_the_anime().await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = get_entry_via(
        &state,
        ProviderKind::MyAnimeList,
        &mal(&server),
        &tokens(),
        "13023",
    )
    .await
    .expect("MAL answered: the anime is not there");
    assert!(got.is_none(), "no entry for an anime MAL does not have");
}

#[tokio::test]
async fn mal_saving_to_an_anime_mal_no_longer_has_writes_nothing() {
    let kitsu = kitsu(KITSU_MAL).await;
    let server = mal_without_the_anime().await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = set_entry_via(
        &state,
        ProviderKind::MyAnimeList,
        &mal(&server),
        &tokens(),
        "13023",
        watched(3),
    )
    .await
    .expect("MAL answered: the anime is not there");
    assert!(got.is_none(), "a save MAL refused as absent is not written");
}

#[tokio::test]
async fn mal_removing_an_anime_mal_no_longer_has_answers_nothing_removed() {
    let kitsu = kitsu(KITSU_MAL).await;
    let server = mal_without_the_anime().await;
    let state = state_with_kitsu(&kitsu.uri());
    let removed = remove_entry_via(
        &state,
        ProviderKind::MyAnimeList,
        &mal(&server),
        &tokens(),
        "13023",
    )
    .await
    .expect("MAL answered: the anime is not there");
    assert!(!removed, "nothing was removed: the route answers 404");
}

#[tokio::test]
async fn mal_a_real_failure_reading_the_anime_still_fails() {
    // Only absence is an answer: a 500 is MAL failing, and stays one.
    let kitsu = kitsu(KITSU_MAL).await;
    let server = MockServer::start().await;
    Mock::given(path("/anime/33778"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let err = get_entry_via(
        &state,
        ProviderKind::MyAnimeList,
        &mal(&server),
        &tokens(),
        "13023",
    )
    .await
    .expect_err("a 500 is no answer");
    assert!(matches!(
        err,
        crate::error::AniError::Upstream { status: 500 }
    ));
}

#[tokio::test]
async fn anilist_reading_a_media_anilist_no_longer_has_answers_no_entry() {
    let kitsu = kitsu(KITSU_ANILIST).await;
    let server = anilist_without_the_media().await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = get_entry_via(
        &state,
        ProviderKind::AniList,
        &anilist(&server),
        &tokens(),
        "13023",
    )
    .await
    .expect("AniList answered: the media is not there");
    assert!(got.is_none(), "no entry for a media AniList does not have");
}

#[tokio::test]
async fn anilist_saving_to_a_media_anilist_no_longer_has_writes_nothing() {
    let kitsu = kitsu(KITSU_ANILIST).await;
    let server = anilist_without_the_media().await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = set_entry_via(
        &state,
        ProviderKind::AniList,
        &anilist(&server),
        &tokens(),
        "13023",
        watched(3),
    )
    .await
    .expect("AniList answered: the media is not there");
    assert!(
        got.is_none(),
        "a save AniList refused as absent is not written"
    );
}

#[tokio::test]
async fn anilist_removing_a_media_anilist_no_longer_has_answers_nothing_removed() {
    let kitsu = kitsu(KITSU_ANILIST).await;
    let server = anilist_without_the_media().await;
    let state = state_with_kitsu(&kitsu.uri());
    let removed = remove_entry_via(
        &state,
        ProviderKind::AniList,
        &anilist(&server),
        &tokens(),
        "13023",
    )
    .await
    .expect("AniList answered: the media is not there");
    assert!(!removed, "nothing was removed: the route answers 404");
}
