use super::*;

fn ep_with(num: u32, thumb: Option<&str>) -> KitsuEpisode {
    KitsuEpisode {
        id: format!("e{num}"),
        canonical_title: Some(format!("Ep {num}")),
        season_number: Some(1),
        number: Some(num),
        relative_number: Some(num),
        length: None,
        synopsis: None,
        airdate: None,
        thumbnail: thumb.map(|t| KitsuEpisodeThumbnail {
            original: Some(t.to_string()),
        }),
    }
}

#[test]
fn merge_thumbs_keeps_kitsu_thumb_when_present() {
    let eps = vec![ep_with(1, Some("https://kitsu.cdn/1.jpg"))];
    let mut anilist = HashMap::new();
    anilist.insert(1u32, "https://crunchyroll.cdn/1.jpg".to_string());
    let merged = merge_thumbs(eps, &anilist);
    assert_eq!(
        merged[0]
            .thumbnail
            .as_ref()
            .unwrap()
            .original
            .as_deref()
            .unwrap(),
        "https://kitsu.cdn/1.jpg"
    );
}

#[test]
fn merge_thumbs_fills_null_thumb_from_anilist() {
    let eps = vec![ep_with(54, None)];
    let mut anilist = HashMap::new();
    anilist.insert(54u32, "https://crunchyroll.cdn/54.jpg".to_string());
    let merged = merge_thumbs(eps, &anilist);
    assert_eq!(
        merged[0]
            .thumbnail
            .as_ref()
            .unwrap()
            .original
            .as_deref()
            .unwrap(),
        "https://crunchyroll.cdn/54.jpg"
    );
}

#[test]
fn merge_thumbs_leaves_null_thumb_when_anilist_missing() {
    // The "both gap" case (One Piece eps 54-61, 131-1010+): Kitsu
    // null, AniList also has nothing. Placeholder still renders.
    let eps = vec![ep_with(131, None)];
    let anilist = HashMap::<u32, String>::new();
    let merged = merge_thumbs(eps, &anilist);
    assert!(merged[0].thumbnail.is_none());
}

#[test]
fn merge_thumbs_handles_mixed_pool() {
    // Realistic shape from a One Piece-shaped probe: ep 53 Kitsu
    // present, ep 54 Kitsu null + AniList null, ep 62 Kitsu null +
    // AniList present, ep 130 ditto.
    let eps = vec![
        ep_with(53, Some("https://kitsu.cdn/53.jpg")),
        ep_with(54, None),
        ep_with(62, None),
        ep_with(130, None),
    ];
    let mut anilist = HashMap::new();
    anilist.insert(62u32, "https://cr.cdn/62.jpg".to_string());
    anilist.insert(130u32, "https://cr.cdn/130.jpg".to_string());
    let merged = merge_thumbs(eps, &anilist);
    assert_eq!(
        merged[0]
            .thumbnail
            .as_ref()
            .unwrap()
            .original
            .as_deref()
            .unwrap(),
        "https://kitsu.cdn/53.jpg"
    );
    assert!(merged[1].thumbnail.is_none());
    assert_eq!(
        merged[2]
            .thumbnail
            .as_ref()
            .unwrap()
            .original
            .as_deref()
            .unwrap(),
        "https://cr.cdn/62.jpg"
    );
    assert_eq!(
        merged[3]
            .thumbnail
            .as_ref()
            .unwrap()
            .original
            .as_deref()
            .unwrap(),
        "https://cr.cdn/130.jpg"
    );
}

#[test]
fn merge_thumbs_skips_eps_without_number() {
    // Kitsu sometimes serves episodes with null `number` (mostly
    // movies / specials in a TV show's listing). No number → no
    // way to match an AniList entry; pass through unchanged.
    let mut ep = ep_with(1, None);
    ep.number = None;
    let eps = vec![ep];
    let mut anilist = HashMap::new();
    anilist.insert(1u32, "https://cr.cdn/1.jpg".to_string());
    let merged = merge_thumbs(eps, &anilist);
    assert!(merged[0].thumbnail.is_none());
}

#[test]
fn merge_thumbs_replaces_thumbnail_with_only_null_original() {
    // Kitsu sometimes serves `thumbnail: { original: null }` —
    // shape-wise present, content-wise empty. Treat it the same as
    // missing thumb and backfill from AniList.
    let mut ep = ep_with(1, None);
    ep.thumbnail = Some(KitsuEpisodeThumbnail { original: None });
    let eps = vec![ep];
    let mut anilist = HashMap::new();
    anilist.insert(1u32, "https://cr.cdn/1.jpg".to_string());
    let merged = merge_thumbs(eps, &anilist);
    assert_eq!(
        merged[0]
            .thumbnail
            .as_ref()
            .unwrap()
            .original
            .as_deref()
            .unwrap(),
        "https://cr.cdn/1.jpg"
    );
}

#[test]
fn cache_anilist_eps_thumbs_caches_success_map_by_kitsu_id() {
    let pool = crate::cache::open_in_memory().expect("in-mem pool");
    let mut map = HashMap::new();
    map.insert(1u32, "https://cr.cdn/1.jpg".to_string());
    cache_anilist_eps_thumbs(&pool, "12", &Ok(map.clone()));
    let body = meta_cache_get(&pool, "anilist:eps-thumbs:v1:k12")
        .expect("cache read")
        .expect("cache hit");
    let back: HashMap<u32, String> = serde_json::from_str(&body).expect("json");
    assert_eq!(back, map);
}

#[test]
fn cache_anilist_eps_thumbs_negative_caches_empty_on_error() {
    // The rate-limit-burst story: AniList 429s on cold load, we
    // negative-cache an empty map so the next 5 minutes of
    // navigation see a cache hit instead of re-burning the budget.
    // The kitsu_id-level key means a subsequent hit also avoids
    // re-running the Kitsu /mappings lookup.
    let pool = crate::cache::open_in_memory().expect("in-mem pool");
    cache_anilist_eps_thumbs(&pool, "12", &Err(()));
    let body = meta_cache_get(&pool, "anilist:eps-thumbs:v1:k12")
        .expect("cache read")
        .expect("cache hit — empty is still a hit");
    let back: HashMap<u32, String> = serde_json::from_str(&body).expect("json");
    assert!(back.is_empty());
}

#[test]
fn anilist_eps_thumbs_key_namespace_separates_kitsu_ids() {
    assert_eq!(anilist_eps_thumbs_key("12"), "anilist:eps-thumbs:v1:k12");
    assert_eq!(anilist_eps_thumbs_key("818"), "anilist:eps-thumbs:v1:k818");
    assert_ne!(anilist_eps_thumbs_key("12"), anilist_eps_thumbs_key("818"));
}

/// Build a state wired to a nowhere-Kitsu so any network call in
/// `thumbs_for_show`'s miss path errors out — exercising the
/// happy hit path without standing up a wiremock.
fn state_for_cache_only_tests() -> AppState {
    state_with_kitsu_at("http://127.0.0.1:1")
}

/// AppState whose Kitsu client points at `kitsu_uri`.
fn state_with_kitsu_at(kitsu_uri: &str) -> AppState {
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::path::PathBuf;
    use std::sync::Arc;
    AppState {
        anidb_base: None,
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/y/history"),
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        hianime_base: None,
        hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        provider_order: vec![crate::scraper::provider::ProviderId::Anidb],
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

#[test]
fn needs_backfill_false_when_every_ep_has_thumb() {
    // The all-Kitsu-thumbs case (e.g. an Attack on Titan page where
    // Kitsu's CDN already covers every ep). The AniList fetch would
    // be unconditional otherwise, burning a Kitsu /mappings round-trip
    // + an AniList rate-limit slot on cold cache with no visible win.
    let eps = vec![
        ep_with(1, Some("https://kitsu.cdn/1.jpg")),
        ep_with(2, Some("https://kitsu.cdn/2.jpg")),
        ep_with(3, Some("https://kitsu.cdn/3.jpg")),
    ];
    assert!(!needs_backfill(&eps));
}

#[test]
fn needs_backfill_true_when_at_least_one_ep_missing_thumb() {
    // One Piece-shaped case: most early eps have Kitsu thumbs, ep 54+
    // are null. AniList lookup is worth running.
    let eps = vec![
        ep_with(1, Some("https://kitsu.cdn/1.jpg")),
        ep_with(54, None),
    ];
    assert!(needs_backfill(&eps));
}

#[test]
fn needs_backfill_true_when_thumb_object_present_but_original_null() {
    // Kitsu sometimes serves `thumbnail: { original: null }` —
    // shape-wise present, content-wise empty. Same as missing.
    let mut ep = ep_with(1, None);
    ep.thumbnail = Some(KitsuEpisodeThumbnail { original: None });
    assert!(needs_backfill(&[ep]));
}

#[test]
fn needs_backfill_skips_eps_without_number() {
    // Eps without a `number` can't be merged regardless of AniList's
    // map (the merge keys by ep number). If those are the only ones
    // missing thumbs, there's nothing to backfill — skip the fetch.
    let mut ep = ep_with(1, None);
    ep.number = None;
    let eps = vec![ep, ep_with(2, Some("https://kitsu.cdn/2.jpg"))];
    assert!(!needs_backfill(&eps));
}

#[test]
fn needs_backfill_false_for_empty_page() {
    // Out-of-range pages return zero episodes. No backfill needed.
    assert!(!needs_backfill(&[]));
}

#[tokio::test]
async fn thumbs_for_show_returns_cached_map_without_network() {
    // The hot path: cache is warm, hit returns instantly with no
    // Kitsu /mappings call and no AniList round-trip. The state's
    // kitsu base is pointed at an unbound port so any miss-path
    // attempt would error visibly — the test passes precisely
    // because the cache-hit branch short-circuits before that.
    let state = state_for_cache_only_tests();
    let mut prefilled = HashMap::new();
    prefilled.insert(1u32, "https://x.cdn/1.jpg".to_string());
    prefilled.insert(2u32, "https://x.cdn/2.jpg".to_string());
    cache_anilist_eps_thumbs(&state.cache_pool, "777", &Ok(prefilled.clone()));
    let got = thumbs_for_show(&state, "777").await;
    assert_eq!(got, prefilled);
}

#[tokio::test]
async fn thumbs_for_show_negative_caches_on_cache_miss_with_unreachable_kitsu() {
    // The miss path: nothing in cache, Kitsu mappings lookup errors
    // (state.kitsu points at an unbound port). `fetch_anilist_eps_thumbs`
    // returns Err, `cache_anilist_eps_thumbs` writes an empty map to
    // the negative cache, and the caller sees the empty result. A
    // second call hits cache and returns instantly.
    let state = state_for_cache_only_tests();
    let first = thumbs_for_show(&state, "999").await;
    assert!(first.is_empty());
    // Verify the negative cache was written.
    let body = meta_cache_get(&state.cache_pool, "anilist:eps-thumbs:v1:k999")
        .expect("cache read")
        .expect("negative cache hit");
    let cached: HashMap<u32, String> = serde_json::from_str(&body).expect("json");
    assert!(cached.is_empty());
}

#[tokio::test]
async fn thumbs_for_show_returns_empty_on_cached_negative() {
    // The negative-cached branch: an earlier failed lookup wrote
    // an empty map. Subsequent calls hit cache and return empty
    // without retrying the network.
    let state = state_for_cache_only_tests();
    cache_anilist_eps_thumbs(&state.cache_pool, "778", &Err(()));
    let got = thumbs_for_show(&state, "778").await;
    assert!(got.is_empty());
}

// --- Which id the AniList lookup uses -----------------------------
//
// Kitsu's mappings decide the query: the MAL id when there is one (the
// path every show that already worked keeps), AniList's own id when
// that is the only mapping, no AniList request at all when neither.

/// Kitsu `/anime/:id?include=mappings` body carrying the given sites.
fn mappings_body(kitsu_id: &str, sites: &[(&str, u32)]) -> String {
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
    serde_json::json!({
        "data": { "id": kitsu_id, "type": "anime", "attributes": { "canonicalTitle": "x" } },
        "included": included,
    })
    .to_string()
}

async fn kitsu_with_mappings(kitsu_id: &str, sites: &[(&str, u32)]) -> wiremock::MockServer {
    use wiremock::matchers::{method, path, query_param};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path(format!("/anime/{kitsu_id}")))
        .and(query_param("include", "mappings"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(mappings_body(kitsu_id, sites)),
        )
        .mount(&kitsu)
        .await;
    kitsu
}

const ONE_EP_BODY: &str = r#"{"data":{"Media":{"streamingEpisodes":[
    {"title":"Episode 1 - Pilot","thumbnail":"https://cr.cdn/1.jpg"}
]}}}"#;

/// AniList mock answering `body` to a query whose variables match
/// `variables`, expecting exactly `times` such requests.
async fn mount_anilist(
    server: &wiremock::MockServer,
    variables: serde_json::Value,
    body: &str,
    times: u64,
) {
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_partial_json(
            serde_json::json!({ "variables": variables }),
        ))
        .respond_with(
            // An `errors` body is AniList's not-found answer, sent as 404.
            wiremock::ResponseTemplate::new(if body.contains("\"errors\"") {
                404
            } else {
                200
            })
            .set_body_string(body.to_string()),
        )
        .expect(times)
        .mount(server)
        .await;
}

fn one_ep_map() -> HashMap<u32, String> {
    HashMap::from([(1u32, "https://cr.cdn/1.jpg".to_string())])
}

#[tokio::test]
async fn a_show_mapped_to_anilist_alone_gets_thumbs_by_anilist_id() {
    let kitsu = kitsu_with_mappings("50551", &[("anilist/anime", 207141)]).await;
    let anilist = wiremock::MockServer::start().await;
    mount_anilist(
        &anilist,
        serde_json::json!({ "id": 207141 }),
        ONE_EP_BODY,
        1,
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = thumbs_for_show_with_anilist_base(&state, "50551", Some(&anilist.uri())).await;
    assert_eq!(got, one_ep_map());
}

#[tokio::test]
async fn a_show_mapped_to_mal_alone_keeps_the_mal_query() {
    let kitsu = kitsu_with_mappings("12", &[("myanimelist/anime", 21)]).await;
    let anilist = wiremock::MockServer::start().await;
    mount_anilist(&anilist, serde_json::json!({ "idMal": 21 }), ONE_EP_BODY, 1).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
    assert_eq!(got, one_ep_map());
}

#[tokio::test]
async fn a_show_mapped_to_both_keeps_the_mal_query() {
    let kitsu =
        kitsu_with_mappings("12", &[("anilist/anime", 21), ("myanimelist/anime", 21)]).await;
    let anilist = wiremock::MockServer::start().await;
    mount_anilist(&anilist, serde_json::json!({ "idMal": 21 }), ONE_EP_BODY, 1).await;
    mount_anilist(&anilist, serde_json::json!({ "id": 21 }), ONE_EP_BODY, 0).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
    assert_eq!(got, one_ep_map());
}

#[tokio::test]
async fn a_show_with_neither_mapping_never_asks_anilist_and_negative_caches() {
    let kitsu = kitsu_with_mappings("7", &[("thetvdb/series", 8)]).await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(ONE_EP_BODY))
        .expect(0)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = thumbs_for_show_with_anilist_base(&state, "7", Some(&anilist.uri())).await;
    assert!(got.is_empty());
    let body = meta_cache_get(&state.cache_pool, "anilist:eps-thumbs:v1:k7")
        .expect("cache read")
        .expect("negative cache hit");
    assert_eq!(body, "{}");
}

#[tokio::test]
async fn anilist_knowing_nothing_by_anilist_id_is_cached_as_empty() {
    // AniList has no media for the id: an empty map, cached under the
    // Kitsu-keyed row so the second visit asks nobody.
    let kitsu = kitsu_with_mappings("50551", &[("anilist/anime", 207141)]).await;
    let anilist = wiremock::MockServer::start().await;
    mount_anilist(
        &anilist,
        serde_json::json!({ "id": 207141 }),
        crate::meta::anilist_media::ANILIST_NOT_FOUND_BODY,
        1,
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let first = thumbs_for_show_with_anilist_base(&state, "50551", Some(&anilist.uri())).await;
    let second = thumbs_for_show_with_anilist_base(&state, "50551", Some(&anilist.uri())).await;
    assert!(first.is_empty());
    assert!(second.is_empty());
}

#[tokio::test]
async fn a_mal_id_anilist_lacks_falls_back_to_the_anilist_id_for_thumbs() {
    let kitsu =
        kitsu_with_mappings("12", &[("anilist/anime", 30), ("myanimelist/anime", 21)]).await;
    let anilist = wiremock::MockServer::start().await;
    mount_anilist(
        &anilist,
        serde_json::json!({ "idMal": 21 }),
        crate::meta::anilist_media::ANILIST_NOT_FOUND_BODY,
        1,
    )
    .await;
    mount_anilist(&anilist, serde_json::json!({ "id": 30 }), ONE_EP_BODY, 1).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
    assert_eq!(got, one_ep_map());
}

#[tokio::test]
async fn both_ids_unknown_to_anilist_cache_empty_thumbs_after_two_requests() {
    let kitsu =
        kitsu_with_mappings("12", &[("anilist/anime", 30), ("myanimelist/anime", 21)]).await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(404)
                .set_body_string(crate::meta::anilist_media::ANILIST_NOT_FOUND_BODY),
        )
        .expect(2)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let first = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
    let second = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
    assert!(first.is_empty());
    assert!(second.is_empty());
}
