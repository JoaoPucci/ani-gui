use super::*;

/// A listing carrying `pairs` as (episode number, thumbnail) entries
/// with empty titles and no episode count.
fn listing(pairs: &[(u32, &str)]) -> StreamingListing {
    StreamingListing {
        entries: pairs
            .iter()
            .map(|(n, url)| {
                (
                    *n,
                    crate::meta::anilist_streaming_eps::StreamingEntry {
                        title: String::new(),
                        thumbnail: Some((*url).to_string()),
                    },
                )
            })
            .collect(),
        ..StreamingListing::default()
    }
}

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
    let anilist = listing(&[(1, "https://crunchyroll.cdn/1.jpg")]);
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
    let anilist = listing(&[(54, "https://crunchyroll.cdn/54.jpg")]);
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
    let anilist = StreamingListing::default();
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
    let anilist = listing(&[
        (62, "https://cr.cdn/62.jpg"),
        (130, "https://cr.cdn/130.jpg"),
    ]);
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
    let anilist = listing(&[(1, "https://cr.cdn/1.jpg")]);
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
    let anilist = listing(&[(1, "https://cr.cdn/1.jpg")]);
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
fn cache_anilist_eps_thumbs_caches_success_listing_by_kitsu_id() {
    let pool = crate::cache::open_in_memory().expect("in-mem pool");
    let map = listing(&[(1, "https://cr.cdn/1.jpg")]);
    cache_anilist_eps_thumbs(&pool, "12", &Ok(map.clone()));
    let body = meta_cache_get(&pool, "anilist:eps-thumbs:v2:k12")
        .expect("cache read")
        .expect("cache hit");
    let back: StreamingListing = serde_json::from_str(&body).expect("json");
    assert_eq!(back, map);
}

#[test]
fn cache_anilist_eps_thumbs_negative_caches_a_show_with_no_ids() {
    // A show Kitsu maps to neither AniList nor MAL has nothing to ask
    // AniList about; it is negative-cached as an empty map so the next
    // 5 minutes of navigation see a cache hit. A lookup that got no
    // answer is not one of these: it is never written
    // (`a_rate_limited_listing_is_not_cached_as_an_empty_one`).
    // The kitsu_id-level key means a subsequent hit also avoids
    // re-running the Kitsu /mappings lookup.
    let pool = crate::cache::open_in_memory().expect("in-mem pool");
    cache_anilist_eps_thumbs(&pool, "12", &Err(()));
    let body = meta_cache_get(&pool, "anilist:eps-thumbs:v2:k12")
        .expect("cache read")
        .expect("cache hit — empty is still a hit");
    let back: StreamingListing = serde_json::from_str(&body).expect("json");
    assert_eq!(back, StreamingListing::default());
}

#[test]
fn anilist_eps_thumbs_key_namespace_separates_kitsu_ids() {
    assert_eq!(anilist_eps_thumbs_key("12"), "anilist:eps-thumbs:v2:k12");
    assert_eq!(anilist_eps_thumbs_key("818"), "anilist:eps-thumbs:v2:k818");
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
    AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/y/history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
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
async fn thumbs_for_show_returns_cached_listing_without_network() {
    // The hot path: cache is warm, hit returns instantly with no
    // Kitsu /mappings call and no AniList round-trip. The state's
    // kitsu base is pointed at an unbound port so any miss-path
    // attempt would error visibly — the test passes precisely
    // because the cache-hit branch short-circuits before that.
    let state = state_for_cache_only_tests();
    let prefilled = listing(&[(1, "https://x.cdn/1.jpg"), (2, "https://x.cdn/2.jpg")]);
    cache_anilist_eps_thumbs(&state.cache_pool, "777", &Ok(prefilled.clone()));
    let got = thumbs_for_show(&state, "777").await;
    assert_eq!(got, prefilled);
}

#[tokio::test]
async fn thumbs_for_show_caches_nothing_when_kitsu_is_unreachable() {
    // The miss path: nothing in cache, Kitsu's mappings lookup errors
    // (state.kitsu points at an unbound port). Kitsu not answering
    // says nothing about which ids the show has, so the caller sees
    // the empty result and nothing is cached: the next view asks
    // again.
    let state = state_for_cache_only_tests();
    let first = thumbs_for_show(&state, "999").await;
    assert!(first.entries.is_empty());
    assert_eq!(
        meta_cache_get(&state.cache_pool, "anilist:eps-thumbs:v2:k999").expect("cache read"),
        None,
        "an unanswered mappings lookup was cached as a show without stills"
    );
}

#[tokio::test]
async fn thumbs_for_show_returns_empty_on_cached_negative() {
    // The negative-cached branch: an earlier lookup found the show
    // mapped to neither tracker and wrote an empty map. Subsequent
    // calls hit cache and return empty without retrying the network.
    let state = state_for_cache_only_tests();
    cache_anilist_eps_thumbs(&state.cache_pool, "778", &Err(()));
    let got = thumbs_for_show(&state, "778").await;
    assert!(got.entries.is_empty());
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

fn one_ep_listing() -> StreamingListing {
    let mut l = listing(&[(1, "https://cr.cdn/1.jpg")]);
    l.entries.get_mut(&1).expect("entry").title = "Pilot".to_string();
    l
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
    assert_eq!(got, one_ep_listing());
}

#[tokio::test]
async fn a_show_mapped_to_mal_alone_keeps_the_mal_query() {
    let kitsu = kitsu_with_mappings("12", &[("myanimelist/anime", 21)]).await;
    let anilist = wiremock::MockServer::start().await;
    mount_anilist(&anilist, serde_json::json!({ "idMal": 21 }), ONE_EP_BODY, 1).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
    assert_eq!(got, one_ep_listing());
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
    assert_eq!(got, one_ep_listing());
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
    assert!(got.entries.is_empty());
    let body = meta_cache_get(&state.cache_pool, "anilist:eps-thumbs:v2:k7")
        .expect("cache read")
        .expect("negative cache hit");
    assert_eq!(
        serde_json::from_str::<StreamingListing>(&body).expect("json"),
        StreamingListing::default()
    );
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
    assert!(first.entries.is_empty());
    assert!(second.entries.is_empty());
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
    assert_eq!(got, one_ep_listing());
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
    assert!(first.entries.is_empty());
    assert!(second.entries.is_empty());
}

// --- A listing that is not the show's -----------------------------

/// AniList's recorded reply for `slug` under
/// `tests/fixtures/episode-thumbs/`.
fn recorded_anilist(slug: &str) -> String {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/episode-thumbs"
    );
    std::fs::read_to_string(format!("{dir}/{slug}.anilist.json")).expect("anilist fixture")
}

/// Kitsu's recorded first episode page for `slug`.
fn recorded_kitsu_page(slug: &str) -> Vec<KitsuEpisode> {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/episode-thumbs"
    );
    let body = std::fs::read(format!("{dir}/{slug}.kitsu-p1.json")).expect("kitsu fixture");
    crate::meta::kitsu::parse_episodes_response(&body).expect("kitsu parses")
}

#[tokio::test]
async fn gintama_dot_keeps_its_placeholders_instead_of_the_2006_stills() {
    // Kitsu 12553 "Gintama." maps to MAL 34096; AniList answers with the
    // 49-entry Crunchyroll listing of the 2006 series. Kitsu has no
    // stills for the twelve episodes, so before the fit rule every one
    // took a 2006 still.
    let kitsu = kitsu_with_mappings("12553", &[("myanimelist/anime", 34096)]).await;
    let anilist = wiremock::MockServer::start().await;
    mount_anilist(
        &anilist,
        serde_json::json!({ "idMal": 34096 }),
        &recorded_anilist("gintama-dot"),
        1,
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let page = recorded_kitsu_page("gintama-dot");
    assert!(needs_backfill(&page));
    let got = thumbs_for_show_with_anilist_base(&state, "12553", Some(&anilist.uri())).await;
    let merged = merge_thumbs(page, &got);
    assert!(
        merged.iter().all(|e| e
            .thumbnail
            .as_ref()
            .and_then(|t| t.original.as_ref())
            .is_none()),
        "a 2006 still landed on Gintama."
    );
}

#[tokio::test]
async fn the_2006_gintama_keeps_the_stills_of_its_own_listing() {
    let kitsu = kitsu_with_mappings("818", &[("myanimelist/anime", 918)]).await;
    let anilist = wiremock::MockServer::start().await;
    mount_anilist(
        &anilist,
        serde_json::json!({ "idMal": 918 }),
        &recorded_anilist("gintama"),
        1,
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let page = recorded_kitsu_page("gintama");
    let got = thumbs_for_show_with_anilist_base(&state, "818", Some(&anilist.uri())).await;
    let merged = merge_thumbs(page, &got);
    assert!(merged.iter().all(|e| e
        .thumbnail
        .as_ref()
        .and_then(|t| t.original.as_ref())
        .is_some()));
}

#[tokio::test]
async fn a_cached_row_that_is_not_a_listing_is_fetched_again() {
    // A row this version cannot read (a hand-edited or truncated
    // body) is a miss: the lookup runs and its listing replaces it.
    let kitsu = kitsu_with_mappings("12", &[("myanimelist/anime", 21)]).await;
    let anilist = wiremock::MockServer::start().await;
    mount_anilist(&anilist, serde_json::json!({ "idMal": 21 }), ONE_EP_BODY, 1).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    meta_cache_put(&state.cache_pool, &anilist_eps_thumbs_key("12"), "[1,2", 60).expect("put");
    let got = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
    assert_eq!(got, one_ep_listing());
}

#[tokio::test]
async fn an_anilist_failure_is_asked_again_rather_than_cached() {
    // A 502 is AniList not answering: each view gets no listing, and
    // the next one asks again.
    let kitsu = kitsu_with_mappings("12", &[("myanimelist/anime", 21)]).await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(502))
        .expect(2)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let first = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
    let second = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
    assert_eq!(first, StreamingListing::default());
    assert_eq!(second, StreamingListing::default());
}

/// An AniList reply listing episodes 1..=`listed` of a finished show
/// counted at `count`, with a still only for the first `with_stills`.
fn listing_body(count: u32, listed: u32, with_stills: u32) -> String {
    let eps: Vec<serde_json::Value> = (1..=listed)
        .map(|n| {
            serde_json::json!({
                "title": format!("Episode {n} - Title {n}"),
                "thumbnail": (n <= with_stills).then(|| format!("https://cr.cdn/{n}.jpg")),
            })
        })
        .collect();
    serde_json::json!({ "data": { "Media": {
        "episodes": count, "status": "FINISHED", "streamingEpisodes": eps,
    } } })
    .to_string()
}

/// An untitled, unstilled Kitsu page of episodes 1..=`count`.
fn untitled_page(count: u32) -> Vec<KitsuEpisode> {
    (1..=count)
        .map(|n| {
            let mut e = ep_with(n, None);
            e.canonical_title = None;
            e
        })
        .collect()
}

#[tokio::test]
async fn entries_without_stills_still_number_the_listing_for_the_fit_rule() {
    // A 12-episode show whose listing runs to 24, stills only on 1-12:
    // the listing is past the count and refused, though every entry
    // that has a still is within it.
    let kitsu = kitsu_with_mappings("12", &[("myanimelist/anime", 21)]).await;
    let anilist = wiremock::MockServer::start().await;
    mount_anilist(
        &anilist,
        serde_json::json!({ "idMal": 21 }),
        &listing_body(12, 24, 12),
        1,
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
    let merged = merge_thumbs(untitled_page(12), &got);
    assert!(merged.iter().all(|e| e
        .thumbnail
        .as_ref()
        .and_then(|t| t.original.as_ref())
        .is_none()));
}

#[tokio::test]
async fn an_entry_without_a_still_fills_nothing() {
    // Within its count the listing fits; episode 2 has no still and
    // keeps its placeholder, episode 1 takes its still.
    let kitsu = kitsu_with_mappings("12", &[("myanimelist/anime", 21)]).await;
    let anilist = wiremock::MockServer::start().await;
    mount_anilist(
        &anilist,
        serde_json::json!({ "idMal": 21 }),
        &listing_body(2, 2, 1),
        1,
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
    let merged = merge_thumbs(untitled_page(2), &got);
    let stills: Vec<Option<&str>> = merged
        .iter()
        .map(|e| e.thumbnail.as_ref().and_then(|t| t.original.as_deref()))
        .collect();
    assert_eq!(stills, vec![Some("https://cr.cdn/1.jpg"), None]);
}

#[tokio::test]
async fn a_later_entry_with_a_still_fills_an_episode_its_first_entry_left_bare() {
    // Subbed and dubbed tracks list the same episode; the first may
    // have no still where the second has one.
    let body = r#"{"data":{"Media":{"episodes":1,"status":"FINISHED","streamingEpisodes":[
        {"title":"Episode 1 - Sub","thumbnail":null},
        {"title":"Episode 1 - Dub","thumbnail":"https://cr.cdn/1-dub.jpg"}
    ]}}}"#;
    let kitsu = kitsu_with_mappings("12", &[("myanimelist/anime", 21)]).await;
    let anilist = wiremock::MockServer::start().await;
    mount_anilist(&anilist, serde_json::json!({ "idMal": 21 }), body, 1).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
    let merged = merge_thumbs(untitled_page(1), &got);
    assert_eq!(
        merged[0]
            .thumbnail
            .as_ref()
            .and_then(|t| t.original.as_deref()),
        Some("https://cr.cdn/1-dub.jpg")
    );
}

#[tokio::test]
async fn a_rate_limited_listing_is_not_cached_as_an_empty_one() {
    // AniList refusing this client says nothing about the show's
    // episodes. Nothing is written, so the next view asks again
    // instead of reading "no stills" for the error TTL.
    let kitsu = kitsu_with_mappings("12", &[("myanimelist/anime", 21)]).await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(429).insert_header("retry-after", "60"))
        .mount(&anilist)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
    assert!(got.entries.is_empty());
    assert_eq!(
        meta_cache_get(&state.cache_pool, "anilist:eps-thumbs:v2:k12").expect("cache read"),
        None,
        "a rate limit was cached as an empty listing"
    );
}

#[tokio::test]
async fn an_anilist_answer_that_cannot_be_read_is_asked_again_rather_than_cached() {
    // A 200 whose body is not AniList's shape says nothing about the
    // show's episodes: each view gets no listing, and the next asks
    // again.
    let kitsu = kitsu_with_mappings("12", &[("myanimelist/anime", 21)]).await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("not json"))
        .expect(2)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    for _ in 0..2 {
        let got = thumbs_for_show_with_anilist_base(&state, "12", Some(&anilist.uri())).await;
        assert_eq!(got, StreamingListing::default());
    }
    assert_eq!(
        meta_cache_get(&state.cache_pool, "anilist:eps-thumbs:v2:k12").expect("cache read"),
        None
    );
}
