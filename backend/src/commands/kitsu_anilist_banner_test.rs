//! AniList's `bannerImage` rides along on requests the app already
//! makes. The trending feed's single `Page` query already asks AniList
//! for every entry's banner; the bridge keeps it on the Kitsu ref
//! instead of dropping it, and remembers it per Kitsu id so the detail
//! page of a show the home page has listed can read it with no AniList
//! request of its own.

use super::*;
use std::path::PathBuf;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DETAIL_FIXTURE: &[u8] =
    include_bytes!("../../../tests/fixtures/kitsu/anime_one_piece_detail.json");

const BANNER: &str = "https://s4.anilist.co/file/anilistcdn/media/anime/banner/21-x.jpg";

fn state_with_kitsu_at(uri: &str) -> AppState {
    AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/y/history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
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

fn anilist_entry(id: u32, mal: u32, banner: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "idMal": mal,
        "title": { "romaji": "x", "english": null, "native": null, "userPreferred": "x" },
        "coverImage": { "extraLarge": null, "large": null, "medium": null, "color": null },
        "bannerImage": banner,
        "status": "RELEASING",
        "episodes": null,
        "trending": 1,
        "averageScore": null,
    })
}

/// Kitsu bridging MAL `mal` to anime `kitsu_id`, whose cover is set.
async fn mount_mapping(kitsu: &MockServer, mal: u32, kitsu_id: &str) {
    let body = serde_json::json!({
        "data": [{
            "id": format!("m-{mal}"),
            "type": "mappings",
            "attributes": { "externalSite": "myanimelist/anime", "externalId": mal.to_string() },
            "relationships": { "item": { "data": { "type": "anime", "id": kitsu_id } } },
        }],
        "included": [{
            "id": kitsu_id,
            "type": "anime",
            "attributes": {
                "canonicalTitle": format!("Show {kitsu_id}"),
                "titles": {},
                "subtype": "TV",
                "status": "current",
                "posterImage": null,
                "coverImage": { "large": format!("https://media.kitsu.app/{kitsu_id}.jpg") },
            },
        }],
    });
    Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalSite]", "myanimelist/anime"))
        .and(query_param("filter[externalId]", mal.to_string()))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(kitsu)
        .await;
}

/// AniList answering the trending `Page` query once, with an entry
/// carrying a banner (MAL 21 → Kitsu 12) and one without (MAL 22 →
/// Kitsu 13).
async fn trending_fixture() -> (MockServer, MockServer) {
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Page": { "media": [
                anilist_entry(101, 21, Some(BANNER)),
                anilist_entry(102, 22, None),
            ] } }
        })))
        .expect(1)
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    mount_mapping(&kitsu, 21, "12").await;
    mount_mapping(&kitsu, 22, "13").await;
    (anilist, kitsu)
}

#[tokio::test]
async fn trending_keeps_each_entrys_anilist_banner_from_its_one_request() {
    let (anilist, kitsu) = trending_fixture().await;
    let state = state_with_kitsu_at(&kitsu.uri());

    let refs = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("trending");

    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].anilist_banner.as_deref(), Some(BANNER));
    assert_eq!(
        refs[1].anilist_banner, None,
        "no banner on AniList stays none"
    );
    // Kitsu's own cover is untouched: the banner rides beside it.
    assert!(refs[0].cover_image.is_some());
    // `.expect(1)` on the AniList mock: one request for the whole feed.
    anilist.verify().await;
}

#[tokio::test]
async fn a_cached_trending_read_asks_anilist_nothing_and_keeps_the_banner() {
    let (anilist, kitsu) = trending_fixture().await;
    let state = state_with_kitsu_at(&kitsu.uri());
    kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("first");

    let again = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("second");

    assert_eq!(again[0].anilist_banner.as_deref(), Some(BANNER));
    anilist.verify().await;
}

#[tokio::test]
async fn the_detail_of_a_listed_show_reads_its_banner_with_no_anilist_request() {
    let (anilist, kitsu) = trending_fixture().await;
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(DETAIL_FIXTURE),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("trending");

    let detail = kitsu_anime_detail_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");

    assert_eq!(detail.anilist_banner.as_deref(), Some(BANNER));
    // Still exactly the trending request: the detail asked nothing.
    anilist.verify().await;
}

#[tokio::test]
async fn the_detail_of_a_show_never_listed_has_no_banner_and_asks_nothing() {
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(DETAIL_FIXTURE),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());

    let detail = kitsu_anime_detail_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");

    assert_eq!(detail.anilist_banner, None);
    anilist.verify().await;
}

#[tokio::test]
async fn a_rate_limited_trending_falls_back_with_no_banner_and_no_retry_storm() {
    // AniList answering 429: the feed falls back to Kitsu's own list,
    // whose refs carry no AniList banner — the hero keeps Kitsu's cover.
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .expect(1..=2)
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "data": [], "included": [] })),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());

    let refs = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("fallback");

    assert!(refs.iter().all(|r| r.anilist_banner.is_none()));
    anilist.verify().await;
}
