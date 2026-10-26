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

/// The seconds the row under `key` is kept for.
fn kept_for(state: &AppState, key: &str) -> u64 {
    let conn = state.cache_pool.get().expect("conn");
    let ttl: i64 = conn
        .query_row(
            "SELECT ttl_seconds FROM meta_cache WHERE key = ?1",
            [key],
            |r| r.get(0),
        )
        .expect("row kept");
    u64::try_from(ttl).expect("non-negative")
}

/// A show Kitsu has no cover for asks AniList for its banner. When that
/// question goes unanswered (AniList failing here), the detail is
/// still served, with the poster treatment, and kept only briefly: a
/// row kept without the banner for its full lifetime would show none
/// for a week, for a failure that said nothing about whether there is
/// one, and a row not kept at all would ask Kitsu and AniList again on
/// every read while AniList fails.
#[tokio::test]
async fn a_banner_lookup_that_goes_unanswered_keeps_the_detail_briefly() {
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .and(query_param("include", "mappings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "id": "12", "type": "anime", "attributes": { "canonicalTitle": "x" } },
            "included": [{
                "id": "1", "type": "mappings",
                "attributes": { "externalSite": "anilist/anime", "externalId": "21" }
            }]
        })))
        .with_priority(1)
        .mount(&kitsu)
        .await;
    let mut uncovered: serde_json::Value =
        serde_json::from_slice(DETAIL_FIXTURE).expect("detail fixture");
    uncovered["data"]["attributes"]["coverImage"] = serde_json::Value::Null;
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&uncovered))
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());

    let detail = kitsu_anime_detail_page_with_anilist_base(&state, "12", Some(&anilist.uri()))
        .await
        .expect("served");

    assert_eq!(detail.anilist_banner, None);
    assert_eq!(
        kept_for(&state, &anime_detail_key("12")),
        crate::cache::ttl::UNANSWERED_TTL.as_secs(),
        "kept only until the question is worth asking again"
    );
}

/// A trending entry whose bridge to Kitsu goes unanswered is left off
/// the strip, but the strip is not kept for its hour as if the entry
/// had no Kitsu entry: it is kept briefly, and the entry asked again.
#[tokio::test]
async fn a_trending_strip_with_an_unanswered_bridge_is_kept_briefly() {
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Page": { "media": [
                anilist_entry(101, 21, Some(BANNER)),
                anilist_entry(102, 22, None),
            ] } }
        })))
        .mount(&anilist)
        .await;
    let kitsu = MockServer::start().await;
    mount_mapping(&kitsu, 21, "12").await;
    // MAL 22's lookup, and the AniList-id lookup after it, fail.
    Mock::given(method("GET"))
        .and(path("/mappings"))
        .respond_with(ResponseTemplate::new(500))
        .with_priority(10)
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());

    let refs = kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
        .await
        .expect("trending");

    assert_eq!(refs.len(), 1, "the unanswered entry is left off");
    assert_eq!(
        kept_for(&state, "kitsu:v6:trending_anilist"),
        crate::cache::ttl::UNANSWERED_TTL.as_secs()
    );
}
