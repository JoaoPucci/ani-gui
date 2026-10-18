//! The episode map as the write-back obtains it: built from the
//! tracker's and Kitsu's answers once, kept in the cache for as long as
//! its evidence warrants, and never kept from a read that failed —
//! against the bodies the services served for Ranma ½, Tower of God's
//! second season and the Attack on Titan Final Chapters.

use std::path::PathBuf;
use std::sync::Arc;

use super::*;
use crate::account::episode_map::{Identity, Shape, TrackerShow};
use crate::cache::{meta_cache_get, meta_cache_row};
use crate::meta::kitsu::ANIME_FIELDS;
use wiremock::matchers::{body_string_contains, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RANMA_ANILIST: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/anilist-media-210.json");
const NETTOU_HEN_ANILIST: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/anilist-media-149939.json");
const FINAL_CHAPTERS_MAL: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/mal-anime-51535.json");
const FINAL_CHAPTERS_MAPPED: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-mal-51535.json");
const TOWER_OF_GOD_MAPPED: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-anilist-153406.json");
const TOWER_OF_GOD_RELATIONS: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-relationships-46312.json");

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
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

fn tokens() -> Tokens {
    Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

fn anilist(api_uri: &str) -> crate::meta::anilist_user::AniListProvider {
    crate::meta::anilist_user::AniListProvider::with_bases(
        reqwest::Client::new(),
        api_uri.to_string(),
        "http://unused-token".to_string(),
    )
}

fn mal(api_uri: &str) -> crate::meta::mal_user::MalProvider {
    crate::meta::mal_user::MalProvider::with_bases(
        reqwest::Client::new(),
        api_uri.to_string(),
        "http://unused-token".to_string(),
        crate::meta::mal_user::MalRefreshState::new(),
    )
}

/// A Kitsu `/anime/:id` detail body with a cover, so the detail read
/// asks AniList for no banner.
fn detail(id: &str, count: Option<u32>, start: &str, end: &str) -> String {
    let count = count.map_or("null".to_string(), |n| n.to_string());
    format!(
        r#"{{"data":{{"id":"{id}","type":"anime","attributes":{{"canonicalTitle":"Show {id}",
            "episodeCount":{count},"startDate":"{start}","endDate":"{end}","status":"finished",
            "subtype":"TV","coverImage":{{"original":"https://media.kitsu.app/c/{id}.jpg"}}}}}}}}"#
    )
}

/// A Kitsu `/anime/:id?include=mappings` body naming the given ids.
fn mappings(id: &str, mal: Option<u32>, anilist: Option<u32>) -> String {
    let mut included = Vec::new();
    if let Some(mal) = mal {
        included.push(format!(
            r#"{{"id":"m{mal}","type":"mappings","attributes":{{"externalSite":"myanimelist/anime","externalId":"{mal}"}}}}"#
        ));
    }
    if let Some(al) = anilist {
        included.push(format!(
            r#"{{"id":"a{al}","type":"mappings","attributes":{{"externalSite":"anilist/anime","externalId":"{al}"}}}}"#
        ));
    }
    format!(
        r#"{{"data":{{"id":"{id}","type":"anime","attributes":{{"canonicalTitle":"Show {id}"}}}},"included":[{}]}}"#,
        included.join(",")
    )
}

/// A Kitsu `media-relationships` body with one related anime.
fn relationship(role: &str, id: &str, count: u32, start: &str, end: &str) -> String {
    format!(
        r#"{{"data":[{{"id":"r1","type":"mediaRelationships","attributes":{{"role":"{role}"}},
            "relationships":{{"destination":{{"data":{{"type":"anime","id":"{id}"}}}}}}}}],
          "included":[{{"id":"{id}","type":"anime","attributes":{{"canonicalTitle":"Show {id}",
            "episodeCount":{count},"startDate":"{start}","endDate":"{end}","status":"finished","subtype":"TV"}}}}],
          "meta":{{"count":1}}}}"#
    )
}

const EMPTY_PAGE: &str = r#"{"data":[],"included":[],"meta":{"count":0}}"#;

async fn kitsu_detail(server: &MockServer, id: &str, body: String, times: u64) {
    Mock::given(method("GET"))
        .and(path(format!("/anime/{id}")))
        .and(query_param("fields[anime]", ANIME_FIELDS))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(times)
        .mount(server)
        .await;
}

async fn kitsu_mapped(server: &MockServer, site: &str, id: u32, body: &str) {
    Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalSite]", site))
        .and(query_param("filter[externalId]", id.to_string()))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

async fn anilist_media(server: &MockServer, id: u32, body: &str) {
    Mock::given(method("POST"))
        .and(body_string_contains("streamingEpisodes"))
        .and(body_string_contains(format!("\"mediaId\":{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(server)
        .await;
}

fn ttl_of(state: &AppState, key: &str) -> u64 {
    meta_cache_row(&state.cache_pool, key)
        .expect("row read")
        .expect("row present")
        .ttl_seconds
}

#[tokio::test]
async fn ranma_on_anilist_is_a_chain_built_once_and_kept_for_the_long_window() {
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "186",
        detail("186", Some(161), "1989-04-15", "1992-09-25"),
        1,
    )
    .await;
    // No Kitsu entry claims Nettou-hen (it has no MAL id to ask by).
    kitsu_mapped(&kitsu, "anilist/anime", 149_939, EMPTY_PAGE).await;
    let anilist_server = MockServer::start().await;
    anilist_media(&anilist_server, 210, RANMA_ANILIST).await;
    anilist_media(&anilist_server, 149_939, NETTOU_HEN_ANILIST).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = anilist(&anilist_server.uri());

    let map = episode_map_for(
        &MapAsk {
            state: &state,
            kind: ProviderKind::AniList,
            provider: &provider,
            tokens: &tokens(),
            anilist_base: Some(&anilist_server.uri()),
        },
        "186",
        ProviderMediaId(210),
        Some(18),
    )
    .await;
    assert_eq!(map.shape, Shape::Chain);
    assert_eq!(
        map.segments
            .iter()
            .map(|s| (s.entry, s.kitsu_from, s.kitsu_to))
            .collect::<Vec<_>>(),
        [(210, 1, 18), (149_939, 19, 161)]
    );
    let key = map_key(ProviderKind::AniList, "186", Some((210, Some(18))));
    assert_eq!(key, "tracker-map:v1:anilist:k186:e210:t18");
    assert_eq!(
        map_key(ProviderKind::MyAnimeList, "49244", None),
        "tracker-map:v1:mal:k49244:unresolved"
    );
    assert_eq!(
        map_key(ProviderKind::MyAnimeList, "7", Some((70, None))),
        "tracker-map:v1:mal:k7:e70:t"
    );
    assert_eq!(ttl_of(&state, &key), TRACKER_MAP_TTL.as_secs());

    // A second write reads the row and asks nothing (every mock expects
    // one request).
    let again = episode_map_for(
        &MapAsk {
            state: &state,
            kind: ProviderKind::AniList,
            provider: &provider,
            tokens: &tokens(),
            anilist_base: Some(&anilist_server.uri()),
        },
        "186",
        ProviderMediaId(210),
        Some(18),
    )
    .await;
    assert_eq!(again, map);
}

#[tokio::test]
async fn agreeing_counts_are_identity_kept_for_the_long_window_with_no_tracker_read() {
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "186",
        detail("186", Some(161), "1989-04-15", "1992-09-25"),
        1,
    )
    .await;
    let mal_server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mal_server)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let map = episode_map_for(
        &MapAsk {
            state: &state,
            kind: ProviderKind::MyAnimeList,
            provider: &mal(&mal_server.uri()),
            tokens: &tokens(),
            anilist_base: None,
        },
        "186",
        ProviderMediaId(210),
        Some(161),
    )
    .await;
    assert_eq!(map, EpisodeMap::identity(Identity::CountsAgree));
    assert_eq!(
        ttl_of(
            &state,
            &map_key(ProviderKind::MyAnimeList, "186", Some((210, Some(161))))
        ),
        TRACKER_MAP_TTL.as_secs()
    );
}

#[tokio::test]
async fn an_unknown_count_is_identity_kept_for_a_day() {
    let kitsu = MockServer::start().await;
    kitsu_detail(&kitsu, "7", detail("7", None, "2026-01-05", ""), 1).await;
    let state = state_with_kitsu(&kitsu.uri());
    let server = MockServer::start().await;
    let map = episode_map_for(
        &MapAsk {
            state: &state,
            kind: ProviderKind::MyAnimeList,
            provider: &mal(&server.uri()),
            tokens: &tokens(),
            anilist_base: None,
        },
        "7",
        ProviderMediaId(70),
        Some(12),
    )
    .await;
    assert_eq!(map, EpisodeMap::identity(Identity::CountUnknown));
    assert_eq!(
        ttl_of(
            &state,
            &map_key(ProviderKind::MyAnimeList, "7", Some((70, Some(12))))
        ),
        TRACKER_MAP_UNSETTLED_TTL.as_secs()
    );
}

#[tokio::test]
async fn a_read_that_fails_is_identity_and_kept_nowhere() {
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .expect(2)
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let server = MockServer::start().await;
    for _ in 0..2 {
        let map = episode_map_for(
            &MapAsk {
                state: &state,
                kind: ProviderKind::MyAnimeList,
                provider: &mal(&server.uri()),
                tokens: &tokens(),
                anilist_base: None,
            },
            "186",
            ProviderMediaId(210),
            Some(18),
        )
        .await;
        assert_eq!(map, EpisodeMap::identity(Identity::Unexplained));
    }
    assert_eq!(
        meta_cache_get(
            &state.cache_pool,
            &map_key(ProviderKind::MyAnimeList, "186", Some((210, Some(18))))
        )
        .expect("read"),
        None
    );
}

#[tokio::test]
async fn a_difference_nothing_explains_is_identity_kept_for_a_day() {
    // Ranma ½ on MyAnimeList with a crafted 100-episode total: no chain
    // sums to Kitsu's 161 (the only sequel is the 6-episode OVA).
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "186",
        detail("186", Some(161), "1989-04-15", "1992-09-25"),
        1,
    )
    .await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 1007, EMPTY_PAGE).await;
    let mal_server = MockServer::start().await;
    let ranma = include_str!("../../../tests/fixtures/tracker-map/wire/mal-anime-210.json")
        .replace("\"num_episodes\":161", "\"num_episodes\":100");
    Mock::given(method("GET"))
        .and(path("/anime/210"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ranma))
        .mount(&mal_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/1007"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"id":1007,"title":"Ranma OVA","num_episodes":6,"status":"finished_airing","media_type":"ova","start_date":"1993-10-21","end_date":"1994-02-18","related_anime":[]}"#,
        ))
        .mount(&mal_server)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let map = episode_map_for(
        &MapAsk {
            state: &state,
            kind: ProviderKind::MyAnimeList,
            provider: &mal(&mal_server.uri()),
            tokens: &tokens(),
            anilist_base: None,
        },
        "186",
        ProviderMediaId(210),
        Some(100),
    )
    .await;
    assert_eq!(map, EpisodeMap::identity(Identity::Unexplained));
    assert_eq!(
        ttl_of(
            &state,
            &map_key(ProviderKind::MyAnimeList, "186", Some((210, Some(100))))
        ),
        TRACKER_MAP_UNSETTLED_TTL.as_secs()
    );
}

#[tokio::test]
async fn the_final_chapters_second_special_on_mal_shares_the_entry_after_the_first() {
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "47132",
        detail("47132", Some(1), "2023-11-04", "2023-11-04"),
        1,
    )
    .await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 51_535, FINAL_CHAPTERS_MAPPED).await;
    // Both specials map to the one MAL entry, so Kitsu's chain of them
    // explains the entry the same way the shared mapping does.
    Mock::given(method("GET"))
        .and(path("/anime/46038"))
        .and(query_param("include", "mappings"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mappings(
            "46038",
            Some(51_535),
            None,
        )))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/46038/media-relationships"))
        .respond_with(ResponseTemplate::new(200).set_body_string(relationship(
            "sequel",
            "47132",
            1,
            "2023-11-04",
            "2023-11-04",
        )))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/47132"))
        .and(query_param("include", "mappings"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mappings(
            "47132",
            Some(51_535),
            None,
        )))
        .mount(&kitsu)
        .await;
    // Kitsu chains the second special after the first, nothing after it.
    Mock::given(method("GET"))
        .and(path("/anime/47132/media-relationships"))
        .respond_with(ResponseTemplate::new(200).set_body_string(relationship(
            "prequel",
            "46038",
            1,
            "2023-03-03",
            "2023-03-03",
        )))
        .mount(&kitsu)
        .await;
    let mal_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/51535"))
        .respond_with(ResponseTemplate::new(200).set_body_string(FINAL_CHAPTERS_MAL))
        .expect(1)
        .mount(&mal_server)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let map = episode_map_for(
        &MapAsk {
            state: &state,
            kind: ProviderKind::MyAnimeList,
            provider: &mal(&mal_server.uri()),
            tokens: &tokens(),
            anilist_base: None,
        },
        "47132",
        ProviderMediaId(51_535),
        Some(2),
    )
    .await;
    assert_eq!(map.shape, Shape::SharedEntry);
    assert_eq!(map.segments.len(), 1);
    assert_eq!(map.segments[0].entry, 51_535);
    assert_eq!(map.segments[0].lead, 1);
    assert_eq!(map.segments[0].entry_total, 2);
}

#[tokio::test]
async fn tower_of_god_s_later_part_is_filed_under_its_prequel_s_anilist_entry() {
    let kitsu = MockServer::start().await;
    // Ours: Koubou-sen, 13 episodes, no AniList entry of its own.
    kitsu_detail(
        &kitsu,
        "49244",
        detail("49244", Some(13), "2024-10-06", "2024-12-29"),
        1,
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/anime/49244/media-relationships"))
        .respond_with(ResponseTemplate::new(200).set_body_string(relationship(
            "prequel",
            "46312",
            13,
            "2024-07-07",
            "2024-09-29",
        )))
        .mount(&kitsu)
        .await;
    // Its prequel maps to AniList 153406 (and MAL 52635); ours to MAL
    // 59989 only, which AniList does not know.
    Mock::given(method("GET"))
        .and(path("/anime/46312"))
        .and(query_param("include", "mappings"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mappings(
            "46312",
            Some(52_635),
            Some(153_406),
        )))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/49244"))
        .and(query_param("include", "mappings"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mappings(
            "49244",
            Some(59_989),
            None,
        )))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/46312/media-relationships"))
        .respond_with(ResponseTemplate::new(200).set_body_string(TOWER_OF_GOD_RELATIONS))
        .mount(&kitsu)
        .await;
    kitsu_mapped(&kitsu, "anilist/anime", 153_406, TOWER_OF_GOD_MAPPED).await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 52_635, TOWER_OF_GOD_MAPPED).await;

    let anilist_server = MockServer::start().await;
    // The public id translations: 52635 → 153406; 59989 → nothing.
    Mock::given(method("POST"))
        .and(body_string_contains("MediaIdByMal"))
        .and(body_string_contains("\"idMal\":52635"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":{"id":153406}}}"#),
        )
        .mount(&anilist_server)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("MediaIdByMal"))
        .and(body_string_contains("\"idMal\":59989"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#))
        .mount(&anilist_server)
        .await;
    anilist_media(
        &anilist_server,
        153_406,
        r#"{"data":{"Media":{"id":153406,"idMal":52635,"episodes":26,"status":"FINISHED","format":"TV",
            "startDate":{"year":2024,"month":7,"day":7},"endDate":{"year":2024,"month":12,"day":29},
            "relations":{"edges":[]},"streamingEpisodes":[]}}}"#,
    )
    .await;

    let state = state_with_kitsu(&kitsu.uri());
    let provider = anilist(&anilist_server.uri());
    let map = episode_map_for_unresolved(
        &MapAsk {
            state: &state,
            kind: ProviderKind::AniList,
            provider: &provider,
            tokens: &tokens(),
            anilist_base: Some(&anilist_server.uri()),
        },
        "49244",
    )
    .await;
    assert_eq!(map.shape, Shape::SplitParts);
    assert_eq!(map.segments.len(), 1);
    assert_eq!(map.segments[0].entry, 153_406);
    assert_eq!(map.segments[0].entry_total, 26);
    assert_eq!(map.segments[0].lead, 13);
    assert_eq!(
        (map.segments[0].kitsu_from, map.segments[0].kitsu_to),
        (1, 13)
    );
    assert_eq!(
        ttl_of(&state, &map_key(ProviderKind::AniList, "49244", None)),
        TRACKER_MAP_TTL.as_secs()
    );
}

#[tokio::test]
async fn a_show_with_no_prequel_that_resolves_is_identity_kept_for_a_day() {
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "5",
        detail("5", Some(12), "2024-01-05", "2024-03-28"),
        1,
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/anime/5/media-relationships"))
        .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_PAGE))
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let server = MockServer::start().await;
    let map = episode_map_for_unresolved(
        &MapAsk {
            state: &state,
            kind: ProviderKind::MyAnimeList,
            provider: &mal(&server.uri()),
            tokens: &tokens(),
            anilist_base: None,
        },
        "5",
    )
    .await;
    assert_eq!(map, EpisodeMap::identity(Identity::Unexplained));
    assert_eq!(
        ttl_of(&state, &map_key(ProviderKind::MyAnimeList, "5", None)),
        TRACKER_MAP_UNSETTLED_TTL.as_secs()
    );
}

#[test]
fn the_window_follows_whether_the_map_is_conclusive() {
    assert_eq!(
        map_ttl(&EpisodeMap::identity(Identity::CountsAgree)),
        TRACKER_MAP_TTL
    );
    assert_eq!(
        map_ttl(&EpisodeMap::identity(Identity::Unexplained)),
        TRACKER_MAP_UNSETTLED_TTL
    );
    assert_eq!(
        map_ttl(&EpisodeMap::identity(Identity::CountUnknown)),
        TRACKER_MAP_UNSETTLED_TTL
    );
    assert!(TRACKER_MAP_UNSETTLED_TTL < TRACKER_MAP_TTL);
}

#[tokio::test]
async fn a_later_part_resolving_to_the_same_entry_is_filed_after_the_parts_before_it() {
    // Shen Yin Wangzuo on MyAnimeList: 52684 is 182 episodes. Kitsu's
    // 46552 (130) maps to it; its sequel 50289 (52) has only an AniList
    // mapping, which AniList records as MAL 52684 too — so both Kitsu
    // parts resolve to the one entry, and the second is its 131st–182nd.
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "50289",
        detail("50289", Some(52), "2025-05-01", "2026-04-23"),
        1,
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/anime/50289/media-relationships"))
        .respond_with(ResponseTemplate::new(200).set_body_string(relationship(
            "prequel",
            "46552",
            130,
            "2022-11-10",
            "2025-04-23",
        )))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/46552/media-relationships"))
        .respond_with(ResponseTemplate::new(200).set_body_string(relationship(
            "sequel",
            "50289",
            52,
            "2025-05-01",
            "2026-04-23",
        )))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/46552"))
        .and(query_param("include", "mappings"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mappings(
            "46552",
            Some(52_684),
            None,
        )))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/50289"))
        .and(query_param("include", "mappings"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mappings(
            "50289",
            None,
            Some(185_729),
        )))
        .mount(&kitsu)
        .await;
    kitsu_mapped(
        &kitsu,
        "myanimelist/anime",
        52_684,
        r#"{"data":[{"id":"m1","type":"mappings","relationships":{"item":{"data":{"type":"anime","id":"46552"}}}}],
            "included":[{"id":"46552","type":"anime","attributes":{"canonicalTitle":"Shen Yin Wangzuo 2",
              "episodeCount":130,"startDate":"2022-11-10","endDate":"2025-04-23","subtype":"ONA"}}]}"#,
    )
    .await;
    let anilist_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains("MalIdByMedia"))
        .and(body_string_contains("185729"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":{"idMal":52684}}}"#),
        )
        .mount(&anilist_server)
        .await;
    let mal_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/52684"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"id":52684,"num_episodes":182,"status":"finished_airing","media_type":"ona",
                "start_date":"2022-11-10","end_date":"2026-04-23","related_anime":[]}"#,
        ))
        .expect(2)
        .mount(&mal_server)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal(&mal_server.uri());
    let tokens = tokens();
    let anilist_uri = anilist_server.uri();
    let ask = MapAsk {
        state: &state,
        kind: ProviderKind::MyAnimeList,
        provider: &provider,
        tokens: &tokens,
        anilist_base: Some(&anilist_uri),
    };
    let map = episode_map_for(&ask, "50289", ProviderMediaId(52_684), Some(182)).await;
    assert_eq!(map.shape, Shape::SplitParts);
    assert_eq!(map.segments[0].lead, 130);
    assert_eq!(map.segments[0].entry_total, 182);
    // And the first part reads the same chain forward: 1–130 of 182.
    let state = state_with_kitsu(&kitsu.uri());
    kitsu_detail(
        &kitsu,
        "46552",
        detail("46552", Some(130), "2022-11-10", "2025-04-23"),
        1,
    )
    .await;
    let ask = MapAsk {
        state: &state,
        ..ask
    };
    let map = episode_map_for(&ask, "46552", ProviderMediaId(52_684), Some(182)).await;
    assert_eq!(map.shape, Shape::SplitParts);
    assert_eq!(map.segments[0].lead, 0);
}

/// A Kitsu `/mappings` body mapping the given Kitsu ids.
fn mapped_body(ids: &[&str]) -> String {
    let data: Vec<String> = ids
        .iter()
        .map(|id| format!(r#"{{"id":"m{id}","type":"mappings","relationships":{{"item":{{"data":{{"type":"anime","id":"{id}"}}}}}}}}"#))
        .collect();
    let included: Vec<String> = ids
        .iter()
        .map(|id| format!(r#"{{"id":"{id}","type":"anime","attributes":{{"canonicalTitle":"Show {id}","episodeCount":12,"startDate":"2020-0{}-01","subtype":"TV"}}}}"#, id.len()))
        .collect();
    format!(
        r#"{{"data":[{}],"included":[{}]}}"#,
        data.join(","),
        included.join(",")
    )
}

#[tokio::test]
async fn kitsu_claims_an_anilist_entry_by_its_own_id_or_by_the_mal_id_it_records() {
    let kitsu = MockServer::start().await;
    kitsu_mapped(&kitsu, "anilist/anime", 1, &mapped_body(&["10"])).await;
    kitsu_mapped(&kitsu, "anilist/anime", 2, EMPTY_PAGE).await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 20, &mapped_body(&["11"])).await;
    kitsu_mapped(&kitsu, "anilist/anime", 3, EMPTY_PAGE).await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 30, EMPTY_PAGE).await;
    let state = state_with_kitsu(&kitsu.uri());
    let show = |id: u32, id_mal: Option<u32>| TrackerShow {
        id,
        id_mal,
        ..Default::default()
    };
    let claimed = |s: TrackerShow| {
        let state = state.clone();
        async move {
            super::kitsu_side::claimed_on_kitsu(&state, ProviderKind::AniList, &s)
                .await
                .expect("read")
        }
    };
    assert!(claimed(show(1, None)).await);
    assert!(claimed(show(2, Some(20))).await);
    assert!(!claimed(show(3, Some(30))).await);
    assert!(!claimed(show(3, None)).await);
}

#[tokio::test]
async fn the_entries_sharing_an_anilist_entry_include_those_mapped_by_its_mal_id_once_each() {
    let kitsu = MockServer::start().await;
    kitsu_mapped(&kitsu, "anilist/anime", 5, &mapped_body(&["40"])).await;
    kitsu_mapped(
        &kitsu,
        "myanimelist/anime",
        50,
        &mapped_body(&["40", "411"]),
    )
    .await;
    let state = state_with_kitsu(&kitsu.uri());
    let entry = TrackerShow {
        id: 5,
        id_mal: Some(50),
        ..Default::default()
    };
    let got = super::kitsu_side::sharing_on_kitsu(&state, ProviderKind::AniList, &entry)
        .await
        .expect("read");
    assert_eq!(
        got.iter().map(|k| k.id.as_str()).collect::<Vec<_>>(),
        ["40", "411"]
    );
}

#[tokio::test]
async fn a_relation_is_single_only_on_a_whole_page_with_one_of_its_role() {
    let kitsu = MockServer::start().await;
    let page = |count: usize, sequels: &[&str]| {
        let data: Vec<String> = sequels
            .iter()
            .map(|id| format!(r#"{{"id":"r{id}","type":"mediaRelationships","attributes":{{"role":"sequel"}},"relationships":{{"destination":{{"data":{{"type":"anime","id":"{id}"}}}}}}}}"#))
            .collect();
        let included: Vec<String> = sequels
            .iter()
            .map(|id| {
                format!(r#"{{"id":"{id}","type":"anime","attributes":{{"episodeCount":12}}}}"#)
            })
            .collect();
        format!(
            r#"{{"data":[{}],"included":[{}],"meta":{{"count":{count}}}}}"#,
            data.join(","),
            included.join(",")
        )
    };
    for (id, body) in [
        ("1", page(1, &["9"])),
        ("2", page(25, &["9"])),
        ("3", page(2, &["8", "9"])),
    ] {
        Mock::given(method("GET"))
            .and(path(format!("/anime/{id}/media-relationships")))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&kitsu)
            .await;
    }
    let state = state_with_kitsu(&kitsu.uri());
    let single = |id: &'static str| {
        let state = state.clone();
        async move {
            super::kitsu_side::single_relation(&state, id, "sequel")
                .await
                .expect("read")
                .map(|k| (k.id, k.episodes))
        }
    };
    assert_eq!(single("1").await, Some(("9".to_string(), Some(12))));
    // A page holding fewer relations than Kitsu counts may hide another.
    assert_eq!(single("2").await, None);
    assert_eq!(single("3").await, None);
}

#[test]
fn the_in_house_tracker_has_no_kitsu_mapping_site() {
    assert!(super::kitsu_side::site(ProviderKind::InHouse).is_err());
    assert_eq!(
        super::kitsu_side::site(ProviderKind::AniList).expect("site"),
        "anilist/anime"
    );
}

#[tokio::test]
async fn a_cached_map_is_built_again_once_the_entry_s_total_moves() {
    // The map Kitsu's 186 has on an 18-episode AniList 210 does not
    // answer for 210 once AniList recounts it. (Kitsu's detail is the
    // detail page's cached row, read once.)
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "186",
        detail("186", Some(161), "1989-04-15", "1992-09-25"),
        1,
    )
    .await;
    kitsu_mapped(&kitsu, "anilist/anime", 149_939, EMPTY_PAGE).await;
    let anilist_server = MockServer::start().await;
    anilist_media(&anilist_server, 210, RANMA_ANILIST).await;
    anilist_media(&anilist_server, 149_939, NETTOU_HEN_ANILIST).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = anilist(&anilist_server.uri());
    let tokens = tokens();
    let ask = MapAsk {
        state: &state,
        kind: ProviderKind::AniList,
        provider: &provider,
        tokens: &tokens,
        anilist_base: None,
    };
    let first = episode_map_for(&ask, "186", ProviderMediaId(210), Some(18)).await;
    assert_eq!(first.shape, Shape::Chain);
    // AniList recounts 210 as 161: the counts agree now, and the old
    // row is not what answers.
    let moved = episode_map_for(&ask, "186", ProviderMediaId(210), Some(161)).await;
    assert_eq!(moved, EpisodeMap::identity(Identity::CountsAgree));
}

#[test]
fn a_map_with_an_entry_still_airing_is_kept_for_the_short_window() {
    use crate::account::episode_map::Segment;
    let airing = EpisodeMap {
        shape: Shape::SplitParts,
        segments: vec![Segment {
            entry: 1,
            entry_total: 26,
            entry_finished: false,
            kitsu_from: 1,
            kitsu_to: 13,
            lead: 13,
            extras_after: vec![],
        }],
    };
    assert_eq!(map_ttl(&airing), TRACKER_MAP_UNSETTLED_TTL);
    let mut done = airing.clone();
    done.segments[0].entry_finished = true;
    assert_eq!(map_ttl(&done), TRACKER_MAP_TTL);
}
