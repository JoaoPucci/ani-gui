//! What one detail-page open costs the trackers' id budget, measured
//! at `resolve_native_media_id` — the step every list-entry read,
//! edit and removal begins with. Extracted via `#[path]` so the
//! fixtures stay off `account.rs`'s CRAP budget.
//!
//! AniList's public API allows this machine 30 requests a minute,
//! shared with trending, airing, thumbnails and the banner backfill.
//! Before these tests, every open of a show Kitsu mapped to one
//! tracker only spent one of them on the same id translation, and
//! asked Kitsu for the same mappings once per connected tracker.

use std::path::PathBuf;
use std::sync::Arc;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::resolve_native_media_id;
use crate::account::provider::{ProviderKind, ProviderMediaId};
use crate::app::AppState;

fn state_with_kitsu(kitsu_uri: &str) -> Arc<AppState> {
    Arc::new(AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui/history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: super::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

/// Kitsu 50497 the way Kitsu answers for it: the AniList mapping and
/// no MAL one.
const KITSU_ANILIST_ONLY: &str = r#"{
    "data": { "id": "50497", "type": "anime", "attributes": { "canonicalTitle": "x" } },
    "included": [{
        "id": "1",
        "type": "mappings",
        "attributes": { "externalSite": "anilist/anime", "externalId": "185660" }
    }]
}"#;

/// A show Kitsu maps to MAL alone.
const KITSU_MAL_ONLY: &str = r#"{
    "data": { "id": "12", "type": "anime", "attributes": { "canonicalTitle": "x" } },
    "included": [{
        "id": "1",
        "type": "mappings",
        "attributes": { "externalSite": "myanimelist/anime", "externalId": "21" }
    }]
}"#;

async fn kitsu_answering_once(kitsu_id: &str, body: &str) -> MockServer {
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/anime/{kitsu_id}")))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(&kitsu)
        .await;
    kitsu
}

async fn anilist_answering_once(body: &str) -> MockServer {
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(&anilist)
        .await;
    anilist
}

#[tokio::test]
async fn opening_an_anilist_only_show_twice_for_mal_costs_one_kitsu_and_one_anilist_request() {
    // Kitsu 50497 with MAL connected: MAL's id comes from AniList's
    // record of the show. Two opens — the second detail page, or the
    // editor's seed read after a write — cost the budget once.
    let kitsu = kitsu_answering_once("50497", KITSU_ANILIST_ONLY).await;
    let anilist = anilist_answering_once(r#"{"data":{"Media":{"idMal":59978}}}"#).await;
    let state = state_with_kitsu(&kitsu.uri());

    for _ in 0..2 {
        let got = resolve_native_media_id(
            &state,
            ProviderKind::MyAnimeList,
            "50497",
            Some(&anilist.uri()),
        )
        .await
        .expect("resolve");
        assert_eq!(got, Some(ProviderMediaId(59978)));
    }
    // Both MockServers verify `.expect(1)` on drop.
}

#[tokio::test]
async fn opening_an_anilist_only_show_twice_for_anilist_asks_anilist_nothing() {
    // The AniList account keys on the id Kitsu already carries; the
    // one request is Kitsu's, and only the first time.
    let kitsu = kitsu_answering_once("50497", KITSU_ANILIST_ONLY).await;
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(0)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());

    for _ in 0..2 {
        let got =
            resolve_native_media_id(&state, ProviderKind::AniList, "50497", Some(&anilist.uri()))
                .await
                .expect("resolve");
        assert_eq!(got, Some(ProviderMediaId(185660)));
    }
}

#[tokio::test]
async fn opening_a_mal_only_show_twice_for_anilist_costs_one_kitsu_and_one_anilist_request() {
    let kitsu = kitsu_answering_once("12", KITSU_MAL_ONLY).await;
    let anilist = anilist_answering_once(r#"{"data":{"Media":{"id":154587}}}"#).await;
    let state = state_with_kitsu(&kitsu.uri());

    for _ in 0..2 {
        let got =
            resolve_native_media_id(&state, ProviderKind::AniList, "12", Some(&anilist.uri()))
                .await
                .expect("resolve");
        assert_eq!(got, Some(ProviderMediaId(154587)));
    }
}

#[tokio::test]
async fn both_trackers_opening_the_same_show_share_one_kitsu_read() {
    // The detail page reads every connected tracker's entry at once;
    // each read resolves the same Kitsu id.
    let kitsu = kitsu_answering_once("50497", KITSU_ANILIST_ONLY).await;
    let anilist = anilist_answering_once(r#"{"data":{"Media":{"idMal":59978}}}"#).await;
    let state = state_with_kitsu(&kitsu.uri());

    let for_anilist =
        resolve_native_media_id(&state, ProviderKind::AniList, "50497", Some(&anilist.uri()))
            .await
            .expect("resolve");
    let for_mal = resolve_native_media_id(
        &state,
        ProviderKind::MyAnimeList,
        "50497",
        Some(&anilist.uri()),
    )
    .await
    .expect("resolve");
    assert_eq!(for_anilist, Some(ProviderMediaId(185660)));
    assert_eq!(for_mal, Some(ProviderMediaId(59978)));
}
