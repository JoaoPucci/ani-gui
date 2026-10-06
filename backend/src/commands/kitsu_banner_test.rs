//! Banner backfill in `kitsu_anime_detail`: a Kitsu detail with a null
//! cover borrows AniList's `bannerImage`, reached through whichever id
//! Kitsu's mappings carry — the MAL id when there is one, AniList's own
//! id otherwise, no AniList request when neither.

use super::*;
use std::path::PathBuf;
use std::sync::Arc;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DETAIL_FIXTURE: &[u8] =
    include_bytes!("../../../tests/fixtures/kitsu/anime_one_piece_detail.json");

fn state_with_kitsu_at(uri: &str) -> AppState {
    AppState {
        anidb_base: None,
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/y/history"),
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        hianime_base: None,
        hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        provider_order: vec![crate::scraper::provider::ProviderId::Anidb],
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// Kitsu answering `/anime/12` with a null-cover detail and its
/// mappings sideload with the given sites.
async fn kitsu_null_cover_show(sites: &[(&str, u32)]) -> MockServer {
    let kitsu = MockServer::start().await;
    let included: Vec<serde_json::Value> = sites
        .iter()
        .enumerate()
        .map(|(i, (site, id))| {
            serde_json::json!({
                "id": (i + 1).to_string(),
                "type": "mappings",
                "attributes": { "externalSite": site, "externalId": id.to_string() },
            })
        })
        .collect();
    let mappings = serde_json::json!({
        "data": { "id": "12", "type": "anime", "attributes": { "canonicalTitle": "x" } },
        "included": included,
    });
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .and(query_param("include", "mappings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mappings))
        .with_priority(1)
        .mount(&kitsu)
        .await;
    let mut detail: serde_json::Value = serde_json::from_slice(DETAIL_FIXTURE).expect("fixture");
    detail["data"]["attributes"]["coverImage"] = serde_json::Value::Null;
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_json(detail),
        )
        .mount(&kitsu)
        .await;
    kitsu
}

/// AniList mock answering `banner` to a query with `variables`,
/// expecting exactly `times` such requests.
async fn mount_banner(server: &MockServer, variables: serde_json::Value, banner: &str, times: u64) {
    let body = serde_json::json!({ "data": { "Media": { "bannerImage": banner } } });
    Mock::given(method("POST"))
        .and(body_partial_json(
            serde_json::json!({ "variables": variables }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(times)
        .mount(server)
        .await;
}

fn banner_of(detail: &KitsuAnimeRef) -> Option<&str> {
    detail.cover_image.as_ref()?.original.as_deref()
}

#[tokio::test]
async fn a_show_mapped_to_anilist_alone_gets_its_banner_by_anilist_id() {
    let kitsu = kitsu_null_cover_show(&[("anilist/anime", 207_141)]).await;
    let anilist = MockServer::start().await;
    mount_banner(
        &anilist,
        serde_json::json!({ "id": 207_141 }),
        "https://al/b.jpg",
        1,
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = kitsu_anime_detail_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");
    assert_eq!(banner_of(&got), Some("https://al/b.jpg"));
}

#[tokio::test]
async fn a_show_mapped_to_mal_alone_keeps_the_mal_query() {
    let kitsu = kitsu_null_cover_show(&[("myanimelist/anime", 21)]).await;
    let anilist = MockServer::start().await;
    mount_banner(
        &anilist,
        serde_json::json!({ "idMal": 21 }),
        "https://al/op.jpg",
        1,
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = kitsu_anime_detail_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");
    assert_eq!(banner_of(&got), Some("https://al/op.jpg"));
}

#[tokio::test]
async fn a_show_mapped_to_both_keeps_the_mal_query() {
    let kitsu = kitsu_null_cover_show(&[("anilist/anime", 21), ("myanimelist/anime", 21)]).await;
    let anilist = MockServer::start().await;
    mount_banner(
        &anilist,
        serde_json::json!({ "idMal": 21 }),
        "https://al/op.jpg",
        1,
    )
    .await;
    mount_banner(
        &anilist,
        serde_json::json!({ "id": 21 }),
        "https://al/wrong.jpg",
        0,
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = kitsu_anime_detail_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");
    assert_eq!(banner_of(&got), Some("https://al/op.jpg"));
}

#[tokio::test]
async fn a_show_with_neither_mapping_never_asks_anilist() {
    let kitsu = kitsu_null_cover_show(&[("thetvdb/series", 8)]).await;
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = kitsu_anime_detail_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");
    assert!(got.cover_image.is_none());
}

#[tokio::test]
async fn anilist_knowing_nothing_by_anilist_id_leaves_the_cover_null_and_caches_it() {
    let kitsu = kitsu_null_cover_show(&[("anilist/anime", 207_141)]).await;
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(
            serde_json::json!({ "variables": { "id": 207_141 } }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#))
        .expect(1)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let first = kitsu_anime_detail_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");
    let second = kitsu_anime_detail_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("cached detail");
    assert!(first.cover_image.is_none());
    assert_eq!(first, second);
}
