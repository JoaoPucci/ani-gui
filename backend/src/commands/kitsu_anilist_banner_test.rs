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

    let detail = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
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

    let detail = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
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

/// Kitsu answering `/anime/12` with a null-cover detail, and its
/// mappings sideload naming MAL 21 — what the null-cover backfill would
/// take to AniList.
async fn mount_null_cover_detail(kitsu: &MockServer) {
    let mappings = serde_json::json!({
        "data": { "id": "12", "type": "anime", "attributes": { "canonicalTitle": "x" } },
        "included": [{
            "id": "1",
            "type": "mappings",
            "attributes": { "externalSite": "myanimelist/anime", "externalId": "21" },
        }],
    });
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .and(query_param("include", "mappings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mappings))
        .with_priority(1)
        .mount(kitsu)
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
        .mount(kitsu)
        .await;
}

#[tokio::test]
async fn a_listed_show_without_a_kitsu_cover_backfills_from_the_remembered_banner() {
    // The null-cover backfill would ask AniList for the very banner
    // the trending request already brought; the remembered one serves.
    let (anilist, kitsu) = trending_fixture().await;
    mount_null_cover_detail(&kitsu).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("trending");

    let detail = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");

    let cover = detail.cover_image.expect("backfilled cover");
    assert_eq!(cover.large.as_deref(), Some(BANNER));
    assert_eq!(detail.anilist_banner.as_deref(), Some(BANNER));
    // Still exactly the trending request.
    anilist.verify().await;
}

#[tokio::test]
async fn a_banner_anilist_no_longer_has_is_forgotten_at_the_next_trending_fetch() {
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Page": { "media": [anilist_entry(101, 21, Some(BANNER))] } }
        })))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&anilist)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Page": { "media": [anilist_entry(101, 21, None)] } }
        })))
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    mount_mapping(&kitsu, 21, "12").await;
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
        .expect("first trending");
    // The feed's own hour runs out; the next fetch asks AniList again.
    crate::cache::meta_cache_delete(&state.cache_pool, "kitsu:v6:trending_anilist")
        .expect("expire trending");
    kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("second trending");

    let detail = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");

    assert_eq!(detail.anilist_banner, None);
}

#[tokio::test]
async fn a_rate_limited_trending_fetch_keeps_the_remembered_banners() {
    // The fallback list is Kitsu's own, whose refs never carry an
    // AniList banner; that is no answer about any banner.
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Page": { "media": [anilist_entry(101, 21, Some(BANNER))] } }
        })))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&anilist)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    mount_mapping(&kitsu, 21, "12").await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{
                "id": "12",
                "type": "anime",
                "attributes": { "canonicalTitle": "Show 12", "titles": {}, "subtype": "TV" },
            }],
        })))
        .mount(&kitsu)
        .await;
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
        .expect("first trending");
    crate::cache::meta_cache_delete(&state.cache_pool, "kitsu:v6:trending_anilist")
        .expect("expire trending");
    let fallback = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("fallback trending");
    assert!(
        fallback.iter().any(|r| r.id == "12"),
        "the fallback lists the show"
    );

    let detail = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");

    assert_eq!(detail.anilist_banner.as_deref(), Some(BANNER));
}

#[tokio::test]
async fn a_null_cover_backfilled_from_anilist_names_the_banner_as_anilists() {
    // A show the trending row never listed: the backfill asks AniList,
    // and the banner it brings is AniList's whichever slot shows it.
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Media": { "bannerImage": BANNER } }
        })))
        .expect(1)
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    mount_null_cover_detail(&kitsu).await;
    let state = state_with_kitsu_at(&kitsu.uri());

    let detail = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");

    assert_eq!(
        detail.cover_image.and_then(|c| c.large).as_deref(),
        Some(BANNER)
    );
    assert_eq!(detail.anilist_banner.as_deref(), Some(BANNER));
    anilist.verify().await;
}

/// Kitsu's detail of anime 12, from the fixture (it has a cover).
async fn mount_detail(kitsu: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(DETAIL_FIXTURE),
        )
        .mount(kitsu)
        .await;
}

#[tokio::test]
async fn a_remembered_banner_the_proxy_found_dead_is_forgotten() {
    let (anilist, kitsu) = trending_fixture().await;
    mount_detail(&kitsu).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("trending");
    // The renderer asked the proxy for the banner; AniList's CDN said
    // it is gone.
    crate::meta::images::mark_dead(&state.cache_pool, BANNER);

    let detail = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");

    assert_eq!(detail.anilist_banner, None);
    assert_eq!(
        crate::cache::meta_cache_get(&state.cache_pool, "anilist:banner:v1:k12").expect("row"),
        None,
        "the row is forgotten, not merely skipped"
    );
    // Still exactly the trending request: a dead banner is not a
    // reason to ask AniList while Kitsu has a cover.
    anilist.verify().await;
}

#[tokio::test]
async fn a_banner_the_proxy_found_dead_is_not_offered_again_while_anilist_still_lists_it() {
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Page": { "media": [anilist_entry(101, 21, Some(BANNER))] } }
        })))
        .expect(2)
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    mount_mapping(&kitsu, 21, "12").await;
    mount_detail(&kitsu).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("first trending");
    crate::meta::images::mark_dead(&state.cache_pool, BANNER);
    // The feed's own hour runs out. AniList still lists the URL, but
    // its CDN is the word on the bytes, and it said they are gone.
    crate::cache::meta_cache_delete(&state.cache_pool, "kitsu:v6:trending_anilist")
        .expect("expire trending");

    let refs = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("second trending");

    assert_eq!(refs[0].anilist_banner, None, "the trending row drops it");
    // Kitsu's own cover is still the row's cover.
    assert!(refs[0].cover_image.is_some());
    let detail = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");
    assert_eq!(detail.anilist_banner, None, "the detail reads none");
    anilist.verify().await;
}

#[tokio::test]
async fn a_null_cover_show_whose_remembered_banner_is_dead_asks_anilist_once_and_refuses_the_same_url(
) {
    // A replaced banner is a new URL, so AniList is still worth asking;
    // the same dead URL back is not a cover to serve.
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Page": { "media": [anilist_entry(101, 21, Some(BANNER))] } }
        })))
        .up_to_n_times(1)
        .with_priority(1)
        .expect(1)
        .mount(&anilist)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Media": { "bannerImage": BANNER } }
        })))
        .expect(1)
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    mount_mapping(&kitsu, 21, "12").await;
    mount_null_cover_detail(&kitsu).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("trending");
    crate::meta::images::mark_dead(&state.cache_pool, BANNER);

    let detail = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");

    assert_eq!(
        detail.cover_image, None,
        "no cover is better than a dead one"
    );
    assert_eq!(detail.anilist_banner, None);
    anilist.verify().await;
}

#[tokio::test]
async fn a_rate_limited_trending_fallback_is_not_kept_as_the_feed() {
    // AniList refusing this client leaves the home page on Kitsu's own
    // list for this load. That list is not AniList's answer, so it is
    // not kept under AniList's feed: the next load asks AniList again.
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "60"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&anilist)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Page": { "media": [anilist_entry(101, 21, Some(BANNER))] } }
        })))
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    mount_mapping(&kitsu, 21, "12").await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{
                "id": "12",
                "type": "anime",
                "attributes": { "canonicalTitle": "Show 12", "titles": {}, "subtype": "TV" },
            }],
        })))
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());

    let fallback = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("fallback");
    assert!(fallback.iter().all(|r| r.anilist_banner.is_none()));

    let next = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("next load");
    assert_eq!(
        next.first().and_then(|r| r.anilist_banner.as_deref()),
        Some(BANNER),
        "the next load read the fallback back instead of asking AniList"
    );
}

#[tokio::test]
async fn a_rate_limited_banner_backfill_does_not_keep_the_detail_without_it() {
    // The backfill's answer is missing, not "no banner": the detail is
    // served without one, but not kept that way.
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "60"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&anilist)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Media": { "bannerImage": BANNER } }
        })))
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    mount_null_cover_detail(&kitsu).await;
    let state = state_with_kitsu_at(&kitsu.uri());

    let first = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");
    assert_eq!(first.anilist_banner, None);

    let next = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail again");
    assert_eq!(
        next.anilist_banner.as_deref(),
        Some(BANNER),
        "the rate-limited detail was kept without its banner"
    );
}

#[tokio::test]
async fn a_banner_backfill_anilist_is_down_for_does_not_keep_the_detail_without_it() {
    // Down is no answer either: the detail is served without a banner
    // and asked again next read.
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .expect(1)
        .with_priority(1)
        .mount(&anilist)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Media": { "bannerImage": BANNER } }
        })))
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    mount_null_cover_detail(&kitsu).await;
    let state = state_with_kitsu_at(&kitsu.uri());

    let first = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail");
    assert_eq!(first.anilist_banner, None);
    let next = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("detail again");
    assert_eq!(next.anilist_banner.as_deref(), Some(BANNER));
}

#[test]
fn a_background_fill_gives_up_its_turn_before_the_episode_page_gives_up_on_it() {
    // The episode route stops waiting for the thumbnail backfill after
    // its enrichment budget. A gate turn taken at the end of a wait as
    // long as that budget would send a request the route then drops:
    // spent against AniList's limit, its answer thrown away.
    assert!(
        crate::meta::anilist_gate::ANILIST.background_wait < ANILIST_ENRICHMENT_BUDGET,
        "{:?} is not under {:?}",
        crate::meta::anilist_gate::ANILIST.background_wait,
        ANILIST_ENRICHMENT_BUDGET
    );
}

/// Kitsu failing its `/mappings` lookups for `ext_id` — under either
/// site — for the first `times` asks.
async fn mount_failing_mappings(kitsu: &MockServer, ext_id: &str, times: u64) {
    Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalId]", ext_id))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(times)
        .with_priority(1)
        .mount(kitsu)
        .await;
}

/// AniList answering every trending ask with `entries`.
async fn trending_answer(entries: Vec<serde_json::Value>) -> MockServer {
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Page": { "media": entries } }
        })))
        .mount(&anilist)
        .await;
    anilist
}

#[tokio::test]
async fn a_feed_kitsu_could_not_bridge_is_not_kept_as_its_fallback() {
    // AniList answers, but Kitsu fails every lookup that would bridge
    // the entry: the home page falls back to Kitsu's own list for that
    // load, and does not keep it under AniList's feed.
    let anilist = trending_answer(vec![anilist_entry(101, 21, Some(BANNER))]).await;
    let kitsu = MockServer::start().await;
    mount_failing_mappings(&kitsu, "21", 1).await;
    mount_failing_mappings(&kitsu, "101", 1).await;
    mount_mapping(&kitsu, 21, "12").await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "data": [], "included": [] })),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());

    let first = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("fallback");
    assert!(first.is_empty(), "{first:?}");

    let next = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("next load");
    assert_eq!(
        next.first().map(|r| r.id.as_str()),
        Some("12"),
        "the next load read the fallback back instead of bridging AniList's feed"
    );
}

#[tokio::test]
async fn a_feed_with_an_entry_kitsu_could_not_bridge_is_not_kept_without_it() {
    // One entry bridges, the other's lookups fail: the row shows the
    // one for this load, and the next load asks for the other again.
    let anilist = trending_answer(vec![
        anilist_entry(101, 21, Some(BANNER)),
        anilist_entry(102, 22, None),
    ])
    .await;
    let kitsu = MockServer::start().await;
    mount_mapping(&kitsu, 21, "12").await;
    mount_failing_mappings(&kitsu, "22", 1).await;
    mount_failing_mappings(&kitsu, "102", 1).await;
    mount_mapping(&kitsu, 22, "13").await;
    let state = state_with_kitsu_at(&kitsu.uri());

    let first = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("trending");
    assert_eq!(
        first.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["12"]
    );

    let next = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("next load");
    assert_eq!(
        next.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["12", "13"],
        "the row was kept without the entry Kitsu failed to bridge"
    );
}

#[tokio::test]
async fn a_trending_entry_kitsu_has_no_cover_for_takes_anilists_banner_as_its_cover() {
    let anilist = trending_answer(vec![anilist_entry(101, 21, Some(BANNER))]).await;
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalId]", "21"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{
                "id": "m-21",
                "type": "mappings",
                "attributes": { "externalSite": "myanimelist/anime", "externalId": "21" },
                "relationships": { "item": { "data": { "type": "anime", "id": "12" } } },
            }],
            "included": [{
                "id": "12",
                "type": "anime",
                "attributes": {
                    "canonicalTitle": "Show 12",
                    "titles": {},
                    "subtype": "TV",
                    "status": "current",
                    "posterImage": null,
                    "coverImage": null,
                },
            }],
        })))
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let refs = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("trending");
    assert_eq!(
        refs[0]
            .cover_image
            .as_ref()
            .and_then(|c| c.large.as_deref()),
        Some(BANNER)
    );
}

#[tokio::test]
async fn an_empty_trending_answer_falls_back_without_keeping_the_fallback() {
    let anilist = trending_answer(Vec::new()).await;
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
    assert!(refs.is_empty());
    assert_eq!(
        crate::cache::meta_cache_get(&state.cache_pool, "kitsu:v6:trending_anilist").expect("read"),
        None
    );
}

#[tokio::test]
async fn an_unreadable_trending_row_is_fetched_again() {
    let (anilist, kitsu) = trending_fixture().await;
    let state = state_with_kitsu_at(&kitsu.uri());
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "kitsu:v6:trending_anilist",
        "not json",
        60,
    )
    .expect("row");
    let refs = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("trending");
    assert_eq!(refs.len(), 2);
}

// --- The last list AniList gave, when it cannot answer -------------------

const TRENDING_KEY: &str = "kitsu:v6:trending_anilist";

/// A trending row AniList gave earlier, past its hour: the body kept,
/// its window over.
fn expire_trending_row(state: &AppState) -> String {
    let body = crate::cache::meta_cache_row(&state.cache_pool, TRENDING_KEY)
        .expect("read")
        .expect("a row AniList gave")
        .body;
    crate::cache::meta_cache_put(&state.cache_pool, TRENDING_KEY, &body, 0).expect("expire");
    assert_eq!(
        crate::cache::meta_cache_get(&state.cache_pool, TRENDING_KEY).expect("read"),
        None,
        "the row is expired"
    );
    body
}

/// Kitsu's own airing list, naming show 99 — what the home page would
/// fall back to with no AniList list at all.
async fn mount_kitsu_airing_list(kitsu: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/anime"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{
                "id": "99",
                "type": "anime",
                "attributes": { "canonicalTitle": "Kitsu's Own", "titles": {}, "subtype": "TV" },
            }],
        })))
        .mount(kitsu)
        .await;
}

#[tokio::test]
async fn an_unanswered_refresh_serves_the_last_list_anilist_gave_and_keeps_it() {
    // AniList answered once; its hour ran out; now it refuses, or is
    // down. The row shows the last list AniList gave — stale, but
    // AniList's — rather than Kitsu's airing list, and the refused
    // refresh leaves that row as it was.
    for refusal in [
        ResponseTemplate::new(429).insert_header("retry-after", "60"),
        ResponseTemplate::new(503),
    ] {
        let anilist = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "Page": { "media": [anilist_entry(101, 21, Some(BANNER))] } }
            })))
            .up_to_n_times(1)
            .with_priority(1)
            .mount(&anilist)
            .await;
        Mock::given(method("POST"))
            .respond_with(refusal)
            .mount(&anilist)
            .await;
        let kitsu = MockServer::start().await;
        mount_mapping(&kitsu, 21, "12").await;
        mount_kitsu_airing_list(&kitsu).await;
        let state = state_with_kitsu_at(&kitsu.uri());
        kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
            .await
            .expect("AniList's answer");
        let kept = expire_trending_row(&state);

        let refs = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
            .await
            .expect("the last list");
        assert_eq!(
            refs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["12"],
            "Kitsu's list stood in for AniList's last one"
        );
        assert_eq!(refs[0].anilist_banner.as_deref(), Some(BANNER));
        let after = crate::cache::meta_cache_row(&state.cache_pool, TRENDING_KEY)
            .expect("read")
            .expect("the row is kept");
        assert_eq!(after.body, kept, "the refused refresh rewrote the row");
    }
}

#[tokio::test]
async fn a_feed_kitsu_cannot_bridge_serves_the_last_list_anilist_gave() {
    // AniList answers, but Kitsu fails every lookup that would bridge
    // it: the last bridged list stands, not Kitsu's airing list.
    let anilist = trending_answer(vec![anilist_entry(101, 21, Some(BANNER))]).await;
    let kitsu = MockServer::start().await;
    mount_mapping(&kitsu, 21, "12").await;
    mount_kitsu_airing_list(&kitsu).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("AniList's answer");
    expire_trending_row(&state);
    kitsu.reset().await;
    mount_failing_mappings(&kitsu, "21", 10).await;
    mount_failing_mappings(&kitsu, "101", 10).await;
    mount_kitsu_airing_list(&kitsu).await;

    let refs = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("the last list");
    assert_eq!(
        refs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["12"]
    );
}

#[tokio::test]
async fn a_stale_list_is_replaced_once_anilist_answers_again() {
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Page": { "media": [anilist_entry(101, 21, Some(BANNER))] } }
        })))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&anilist)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .with_priority(2)
        .mount(&anilist)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Page": { "media": [anilist_entry(102, 22, None)] } }
        })))
        .with_priority(3)
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    mount_mapping(&kitsu, 21, "12").await;
    mount_mapping(&kitsu, 22, "13").await;
    mount_kitsu_airing_list(&kitsu).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("first answer");
    expire_trending_row(&state);
    let stale = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("stale");
    assert_eq!(stale[0].id, "12");

    let fresh = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("fresh");
    assert_eq!(
        fresh.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["13"]
    );
    let row = crate::cache::meta_cache_get(&state.cache_pool, TRENDING_KEY)
        .expect("read")
        .expect("the new answer is kept");
    assert!(row.contains("\"13\""), "{row}");
}

#[tokio::test]
async fn a_stale_list_does_not_offer_a_banner_since_found_gone() {
    // The last list carries AniList's banner, as the hero's image and as
    // the cover it filled in for a show Kitsu had none for. The image
    // proxy has since found that URL gone: the stale list is served
    // without it, as a fresh answer would have been.
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Page": { "media": [anilist_entry(101, 21, Some(BANNER))] } }
        })))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&anilist)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalId]", "21"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{
                "id": "m-21",
                "type": "mappings",
                "attributes": { "externalSite": "myanimelist/anime", "externalId": "21" },
                "relationships": { "item": { "data": { "type": "anime", "id": "12" } } },
            }],
            "included": [{
                "id": "12",
                "type": "anime",
                "attributes": {
                    "canonicalTitle": "Show 12",
                    "titles": {},
                    "subtype": "TV",
                    "status": "current",
                    "posterImage": null,
                    "coverImage": null,
                },
            }],
        })))
        .mount(&kitsu)
        .await;
    mount_kitsu_airing_list(&kitsu).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let first = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("AniList's answer");
    assert_eq!(
        first[0]
            .cover_image
            .as_ref()
            .and_then(|c| c.large.as_deref()),
        Some(BANNER)
    );
    expire_trending_row(&state);
    crate::meta::images::mark_dead(&state.cache_pool, BANNER);

    let stale = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("the last list");
    assert_eq!(stale[0].id, "12");
    assert_eq!(stale[0].anilist_banner, None, "a gone banner was offered");
    assert!(
        stale[0].cover_image.is_none(),
        "a gone banner was offered as the cover"
    );
}
