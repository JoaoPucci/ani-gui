//! Kitsu, read without a login, hides every entry it marks `nsfw`:
//! search and browse never list one and `/anime/<id>` answers 404. Its
//! mappings lookup by MAL or AniList id still names such an entry,
//! inline. The rows built on that lookup leave those entries out, so
//! they list what search and browse list and no card leads to a page
//! Kitsu refuses.

use std::sync::Arc;

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::app::AppState;
use crate::cache::families::ANILIST_TRENDING;
use crate::meta::kitsu::KitsuAnimeRef;

/// Redo of Healer: MAL 40750, Kitsu 42732, which the mappings lookup
/// serves with `nsfw: true` and `ageRating: "R18"`, and `/anime/42732`
/// answers with 404.
const ADULT_MAL: u32 = 40_750;
const ADULT_ANILIST: u32 = 112_444;
const ADULT_KITSU: &str = "42732";

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
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
        flags: crate::flags::FlagStore::fixed(crate::flags::DECLARED),
    });
    dirs.hold(state)
}

/// Kitsu's anime resource as the mappings lookup inlines it, rated
/// `age_rating` and flagged `nsfw` the way Kitsu flags that rating.
fn anime(kitsu_id: &str, age_rating: &str) -> serde_json::Value {
    serde_json::json!({
        "id": kitsu_id,
        "type": "anime",
        "attributes": {
            "canonicalTitle": format!("Show {kitsu_id}"),
            "titles": {},
            "subtype": "TV",
            "status": "finished",
            "ageRating": age_rating,
            "nsfw": age_rating == "R18",
            "posterImage": null,
            "coverImage": { "large": format!("https://media.kitsu.app/{kitsu_id}.jpg") },
        },
    })
}

/// Kitsu mapping `site`'s `external` id to `kitsu_id`, rated
/// `age_rating`.
async fn mount_mapping(
    kitsu: &MockServer,
    site: &str,
    external: u32,
    kitsu_id: &str,
    age_rating: &str,
) {
    Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalSite]", site))
        .and(query_param("filter[externalId]", external.to_string()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{
                "id": format!("m-{external}"),
                "type": "mappings",
                "attributes": { "externalSite": site, "externalId": external.to_string() },
                "relationships": { "item": { "data": { "type": "anime", "id": kitsu_id } } },
            }],
            "included": [anime(kitsu_id, age_rating)],
        })))
        .mount(kitsu)
        .await;
}

/// The anime lookup Kitsu refuses for an entry it hides.
async fn mount_hidden_detail(kitsu: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/anime/{ADULT_KITSU}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(kitsu)
        .await;
}

fn anilist_entry(id: u32, mal: u32) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "idMal": mal,
        "title": { "romaji": "x", "english": null, "native": null, "userPreferred": "x" },
        "coverImage": { "extraLarge": null, "large": null, "medium": null, "color": null },
        "bannerImage": null,
        "status": "FINISHED",
        "episodes": 12,
        "trending": 1,
        "averageScore": null,
    })
}

/// Kitsu whose mappings answer MAL 21 with an ordinary show (Kitsu 12,
/// PG) and MAL 40750 with Redo of Healer (R18), and whose AniList-id
/// mapping is never to be asked: the MAL lookup already answered both.
async fn kitsu_with_an_adult_show() -> MockServer {
    let kitsu = MockServer::start().await;
    mount_mapping(&kitsu, "myanimelist/anime", 21, "12", "PG").await;
    mount_mapping(&kitsu, "myanimelist/anime", ADULT_MAL, ADULT_KITSU, "R18").await;
    Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalSite]", "anilist/anime"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": [] })))
        .expect(0)
        .mount(&kitsu)
        .await;
    mount_hidden_detail(&kitsu).await;
    kitsu
}

fn ids(refs: &[KitsuAnimeRef]) -> Vec<&str> {
    refs.iter().map(|r| r.id.as_str()).collect()
}

// --- Trending -----------------------------------------------------------

/// AniList's trending feed listing Redo of Healer first and an
/// ordinary show second, answered `times` times.
async fn anilist_trending(times: u64) -> MockServer {
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "Page": { "media": [
                anilist_entry(ADULT_ANILIST, ADULT_MAL),
                anilist_entry(101, 21),
            ] } }
        })))
        .expect(times)
        .mount(&anilist)
        .await;
    anilist
}

#[tokio::test]
async fn trending_leaves_out_a_show_kitsu_hides_as_adult() {
    let kitsu = kitsu_with_an_adult_show().await;
    // Asked once: the row without the adult show is AniList's whole
    // answer, kept for the hour like any other.
    let anilist = anilist_trending(1).await;
    let state = state_with_kitsu(&kitsu.uri());

    let first =
        crate::commands::kitsu::kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
            .await
            .expect("trending");
    let again =
        crate::commands::kitsu::kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
            .await
            .expect("trending, kept");

    assert_eq!(ids(&first), ["12"]);
    assert_eq!(ids(&again), ["12"]);
    anilist.verify().await;
    // `.expect(0)` on the AniList-id mapping: the MAL lookup's answer
    // already said what the show is, so it is not asked again by
    // another id.
    kitsu.verify().await;
}

/// A trending row as a build before this one kept it: Redo of Healer
/// among AniList's answer.
fn kept_row_with_an_adult_show() -> String {
    let refs: Vec<KitsuAnimeRef> = [("12", "PG"), (ADULT_KITSU, "R18")]
        .into_iter()
        .map(|(id, rating)| {
            serde_json::from_value(serde_json::json!({
                "id": id,
                "canonical_title": format!("Show {id}"),
                "age_rating": rating,
            }))
            .expect("a ref")
        })
        .collect();
    serde_json::to_string(&refs).expect("a row")
}

#[tokio::test]
async fn a_kept_trending_row_is_served_without_a_show_kitsu_hides_as_adult() {
    let kitsu = MockServer::start().await;
    let anilist = anilist_trending(0).await;
    let state = state_with_kitsu(&kitsu.uri());
    crate::cache::meta_cache_put(
        &state.cache_pool,
        ANILIST_TRENDING,
        &kept_row_with_an_adult_show(),
        3600,
    )
    .expect("row");

    let refs =
        crate::commands::kitsu::kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
            .await
            .expect("trending");

    assert_eq!(ids(&refs), ["12"]);
}

#[tokio::test]
async fn a_stale_trending_row_is_served_without_a_show_kitsu_hides_as_adult() {
    // AniList is down, so the last list it gave stands, past its hour.
    let kitsu = MockServer::start().await;
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1..)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    crate::cache::meta_cache_put(
        &state.cache_pool,
        ANILIST_TRENDING,
        &kept_row_with_an_adult_show(),
        0,
    )
    .expect("an expired row");

    let refs =
        crate::commands::kitsu::kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
            .await
            .expect("trending");

    assert_eq!(ids(&refs), ["12"]);
}

// --- Watch Later --------------------------------------------------------

/// AniList, which the Watch Later bridge asks only for a MAL id Kitsu
/// did not map: never, here.
async fn anilist_never_asked() -> MockServer {
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&anilist)
        .await;
    anilist
}

#[tokio::test]
async fn watch_later_leaves_out_a_show_kitsu_hides_as_adult() {
    let kitsu = kitsu_with_an_adult_show().await;
    let anilist = anilist_never_asked().await;
    let state = state_with_kitsu(&kitsu.uri());

    let cards = crate::commands::account::kitsu_for_mal_ids_with_anilist_base(
        &state,
        vec![ADULT_MAL, 21],
        Some(&anilist.uri()),
    )
    .await;

    assert_eq!(ids(&cards), ["12"]);
    // Kitsu answered what the show is: neither AniList nor Kitsu's
    // AniList-id mapping is asked about it.
    anilist.verify().await;
    kitsu.verify().await;
}

#[tokio::test]
async fn watch_later_leaves_out_an_adult_show_it_resolves_from_cache() {
    // The first load remembers the mapping and keeps the card's detail,
    // as every build has; the next load resolves both from the cache,
    // the way a rail whose cache a build before this one filled does.
    let kitsu = kitsu_with_an_adult_show().await;
    let anilist = anilist_never_asked().await;
    let state = state_with_kitsu(&kitsu.uri());
    let anilist_uri = anilist.uri();
    let load = || {
        crate::commands::account::kitsu_for_mal_ids_with_anilist_base(
            &state,
            vec![ADULT_MAL, 21],
            Some(&anilist_uri),
        )
    };
    load().await;

    let cards = load().await;

    assert_eq!(ids(&cards), ["12"]);
    let mappings = kitsu
        .received_requests()
        .await
        .expect("requests")
        .iter()
        .filter(|r| r.url.path() == "/mappings")
        .count();
    assert_eq!(
        mappings, 2,
        "the second load reads both mappings from the cache"
    );
}

#[tokio::test]
async fn watch_later_leaves_out_an_adult_show_whose_kept_detail_has_expired() {
    // Remembered mapping, detail past its window: the read asks Kitsu,
    // which refuses the hidden entry.
    let kitsu = kitsu_with_an_adult_show().await;
    let anilist = anilist_never_asked().await;
    let state = state_with_kitsu(&kitsu.uri());
    crate::cache::meta_cache_put(
        &state.cache_pool,
        &format!("{}{ADULT_MAL}", crate::cache::families::KITSU_MAL_MAP),
        ADULT_KITSU,
        3600,
    )
    .expect("a remembered mapping");

    let cards = crate::commands::account::kitsu_for_mal_ids_with_anilist_base(
        &state,
        vec![ADULT_MAL, 21],
        Some(&anilist.uri()),
    )
    .await;

    assert_eq!(ids(&cards), ["12"]);
}
