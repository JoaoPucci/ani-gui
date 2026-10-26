//! The episode map as the write-back obtains it: built from the
//! tracker's and Kitsu's answers once, kept in the cache for as long as
//! its evidence warrants, and never kept from a read that failed —
//! against the bodies the services served for Ranma ½, Tower of God's
//! second season and the Attack on Titan Final Chapters.

use std::path::PathBuf;
use std::sync::Arc;

use super::*;
use crate::account::episode_map::{Identity, KitsuShow, Shape, TrackerShow};
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
    // Each anime the page lists is mapped to the id it was listed for,
    // unless a case says otherwise: the ownership check reads its
    // mappings as the write's own id resolution does.
    let page: serde_json::Value = serde_json::from_str(body).expect("a mappings page");
    for anime in page["included"].as_array().into_iter().flatten() {
        if anime["type"] != "anime" {
            continue;
        }
        let anime_id = anime["id"].as_str().expect("an id");
        let (mal, anilist) = match site {
            "myanimelist/anime" => (Some(id), None),
            _ => (None, Some(id)),
        };
        Mock::given(method("GET"))
            .and(path(format!("/anime/{anime_id}")))
            .and(query_param("include", "mappings"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(mappings(anime_id, mal, anilist)),
            )
            .with_priority(10)
            .mount(server)
            .await;
    }
    no_anilist_answer(server).await;
}

/// AniList, served from `server`, answering that it indexes nothing a
/// case does not answer for itself.
async fn no_anilist_answer(server: &MockServer) {
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#))
        .with_priority(10)
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
    no_anilist_answer(server).await;
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
    .await
    .expect("the map");
    assert_eq!(map.shape, Shape::Chain);
    assert_eq!(
        map.segments
            .iter()
            .map(|s| (s.entry, s.kitsu_from, s.kitsu_to))
            .collect::<Vec<_>>(),
        [(210, 1, 18), (149_939, 19, 161)]
    );
    let key = map_key(ProviderKind::AniList, "186", Some((210, Some(18))));
    assert_eq!(key, "tracker-map:v2:anilist:k186:e210:t18");
    assert_eq!(
        map_key(ProviderKind::MyAnimeList, "49244", None),
        "tracker-map:v2:mal:k49244:unresolved"
    );
    assert_eq!(
        map_key(ProviderKind::MyAnimeList, "7", Some((70, None))),
        "tracker-map:v2:mal:k7:e70:t"
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
    .await
    .expect("the map");
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
            anilist_base: Some(&kitsu.uri()),
        },
        "186",
        ProviderMediaId(210),
        Some(161),
    )
    .await
    .expect("the map");
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
            anilist_base: Some(&kitsu.uri()),
        },
        "7",
        ProviderMediaId(70),
        Some(12),
    )
    .await
    .expect("the map");
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
async fn a_read_that_fails_fails_the_map_and_is_kept_nowhere() {
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .expect(2)
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let server = MockServer::start().await;
    for _ in 0..2 {
        let err = episode_map_for(
            &MapAsk {
                state: &state,
                kind: ProviderKind::MyAnimeList,
                provider: &mal(&server.uri()),
                tokens: &tokens(),
                anilist_base: Some(&kitsu.uri()),
            },
            "186",
            ProviderMediaId(210),
            Some(18),
        )
        .await
        .expect_err("a read that failed says nothing about the show");
        assert!(
            matches!(err, crate::error::AniError::Upstream { status: 503 }),
            "{err:?}"
        );
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

/// Ranma ½ on AniList with Kitsu's detail served and the entry's own
/// evidence read answered by `entry`, and Nettou-hen's by `sequel`.
async fn ranma_with(entry: ResponseTemplate, sequel: ResponseTemplate) -> (MockServer, MockServer) {
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
    Mock::given(method("POST"))
        .and(body_string_contains("streamingEpisodes"))
        .and(body_string_contains("\"mediaId\":210"))
        .respond_with(entry)
        .mount(&anilist_server)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("streamingEpisodes"))
        .and(body_string_contains("\"mediaId\":149939"))
        .respond_with(sequel)
        .mount(&anilist_server)
        .await;
    (kitsu, anilist_server)
}

#[tokio::test]
async fn a_rate_limited_evidence_read_fails_the_map_with_its_wait() {
    let (kitsu, anilist_server) = ranma_with(
        ResponseTemplate::new(429).insert_header("retry-after", "30"),
        ResponseTemplate::new(200).set_body_string(NETTOU_HEN_ANILIST),
    )
    .await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = anilist(&anilist_server.uri());
    let err = episode_map_for(
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
    .await
    .expect_err("a rate-limited read is no answer");
    assert!(
        matches!(
            err,
            crate::error::AniError::RateLimited {
                retry_after_secs: Some(30)
            }
        ),
        "{err:?}"
    );
    assert_eq!(
        meta_cache_get(
            &state.cache_pool,
            &map_key(ProviderKind::AniList, "186", Some((210, Some(18))))
        )
        .expect("read"),
        None
    );
}

#[tokio::test]
async fn a_read_that_fails_partway_along_the_chain_fails_the_map() {
    for (sequel, expected) in [
        (ResponseTemplate::new(500), "upstream"),
        (
            ResponseTemplate::new(200).set_body_string("<html>busy</html>"),
            "unreadable",
        ),
    ] {
        let (kitsu, anilist_server) = ranma_with(
            ResponseTemplate::new(200).set_body_string(RANMA_ANILIST),
            sequel,
        )
        .await;
        let state = state_with_kitsu(&kitsu.uri());
        let provider = anilist(&anilist_server.uri());
        let err = episode_map_for(
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
        .await
        .expect_err("a chain read only in part is no answer");
        match expected {
            "upstream" => assert!(
                matches!(err, crate::error::AniError::Upstream { status: 500 }),
                "{err:?}"
            ),
            _ => assert!(
                matches!(err, crate::error::AniError::ParseFailed { .. }),
                "{err:?}"
            ),
        }
        assert_eq!(
            meta_cache_get(
                &state.cache_pool,
                &map_key(ProviderKind::AniList, "186", Some((210, Some(18))))
            )
            .expect("read"),
            None
        );
    }
}

#[tokio::test]
async fn a_sequel_the_tracker_no_longer_serves_ends_the_chain_as_an_answer() {
    // The tracker names a sequel and then answers that it has no such
    // entry: that is an answer, not a failed read, and it leaves the
    // chain short of Kitsu's count, so the map is identity, kept for
    // the short window.
    let (kitsu, anilist_server) = ranma_with(
        ResponseTemplate::new(200).set_body_string(RANMA_ANILIST),
        ResponseTemplate::new(404).set_body_string(
            r#"{"errors":[{"message":"Not Found.","status":404}],"data":{"Media":null}}"#,
        ),
    )
    .await;
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
    .await
    .expect("a sequel the tracker says is gone is an answer");
    assert_eq!(map, EpisodeMap::identity(Identity::Unexplained));
    assert_eq!(
        ttl_of(
            &state,
            &map_key(ProviderKind::AniList, "186", Some((210, Some(18))))
        ),
        TRACKER_MAP_UNSETTLED_TTL.as_secs()
    );
}

#[tokio::test]
async fn an_entry_the_tracker_no_longer_serves_leaves_an_unresolved_show_identity() {
    // Kitsu files show 5 after its prequel 4, which Kitsu maps to MAL 40;
    // MAL answers that 40 is gone. That is an answer: the show is filed
    // under nothing, identity for the short window, rather than a write
    // that fails on every watch.
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
        .respond_with(ResponseTemplate::new(200).set_body_string(relationship(
            "prequel",
            "4",
            12,
            "2023-01-05",
            "2023-03-28",
        )))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/4"))
        .and(query_param("include", "mappings"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mappings("4", Some(40), None)))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/4/media-relationships"))
        .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_PAGE))
        .mount(&kitsu)
        .await;
    let mal_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/40"))
        .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"error":"not_found"}"#))
        .expect(1)
        .mount(&mal_server)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let map = episode_map_for_unresolved(
        &MapAsk {
            state: &state,
            kind: ProviderKind::MyAnimeList,
            provider: &mal(&mal_server.uri()),
            tokens: &tokens(),
            anilist_base: Some(&kitsu.uri()),
        },
        "5",
    )
    .await
    .expect("an entry the tracker says is gone is an answer");
    assert_eq!(map, EpisodeMap::identity(Identity::Unexplained));
    assert_eq!(
        ttl_of(&state, &map_key(ProviderKind::MyAnimeList, "5", None)),
        TRACKER_MAP_UNSETTLED_TTL.as_secs()
    );
}

#[tokio::test]
async fn a_prequel_read_that_fails_fails_the_unresolved_map() {
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
        .respond_with(ResponseTemplate::new(503))
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let server = MockServer::start().await;
    let err = episode_map_for_unresolved(
        &MapAsk {
            state: &state,
            kind: ProviderKind::MyAnimeList,
            provider: &mal(&server.uri()),
            tokens: &tokens(),
            anilist_base: Some(&kitsu.uri()),
        },
        "5",
    )
    .await
    .expect_err("an unread prequel is no answer");
    assert!(
        matches!(err, crate::error::AniError::Upstream { status: 503 }),
        "{err:?}"
    );
    assert_eq!(
        meta_cache_get(
            &state.cache_pool,
            &map_key(ProviderKind::MyAnimeList, "5", None)
        )
        .expect("read"),
        None
    );
}

#[tokio::test]
async fn ranma_s_nineteenth_episode_is_not_written_while_the_evidence_is_rate_limited() {
    let (kitsu, anilist_server) = ranma_with(
        ResponseTemplate::new(429).insert_header("retry-after", "30"),
        ResponseTemplate::new(200).set_body_string(NETTOU_HEN_ANILIST),
    )
    .await;
    Mock::given(method("POST"))
        .and(body_string_contains("query Tracked"))
        .and(body_string_contains("\"mediaId\":210"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Media":{"episodes":18,"mediaListEntry":{"status":"CURRENT","progress":16}}}}"#,
        ))
        .mount(&anilist_server)
        .await;
    // A write sent anyway is answered, so the case shows it went out.
    Mock::given(method("POST"))
        .and(body_string_contains("SaveMediaListEntry"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"SaveMediaListEntry":{"mediaId":210,"status":"CURRENT","progress":18,"score":0,"updatedAt":1,"repeat":0,"media":{"idMal":null,"title":{"romaji":"Ranma","english":null,"userPreferred":"Ranma"}}}}}"#,
        ))
        .mount(&anilist_server)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = anilist(&anilist_server.uri());
    let err = crate::commands::account_mapped::push_progress_mapped(
        &MapAsk {
            state: &state,
            kind: ProviderKind::AniList,
            provider: &provider,
            tokens: &tokens(),
            anilist_base: Some(&anilist_server.uri()),
        },
        "186",
        Some(ProviderMediaId(210)),
        crate::account::provider::EntryUpdate {
            progress_episodes: Some(19),
            ..Default::default()
        },
    )
    .await
    .expect_err("the write waits for the map");
    assert!(
        matches!(
            err,
            crate::error::AniError::RateLimited {
                retry_after_secs: Some(30)
            }
        ),
        "{err:?}"
    );
    let saves: Vec<String> = anilist_server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| String::from_utf8_lossy(&r.body).into_owned())
        .filter(|b| b.contains("SaveMediaListEntry"))
        .collect();
    assert!(saves.is_empty(), "nothing is written: {saves:?}");
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
            anilist_base: Some(&kitsu.uri()),
        },
        "186",
        ProviderMediaId(210),
        Some(100),
    )
    .await
    .expect("the map");
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
            anilist_base: Some(&kitsu.uri()),
        },
        "47132",
        ProviderMediaId(51_535),
        Some(2),
    )
    .await
    .expect("the map");
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
    // The season's own prequel, the first season, has an AniList entry
    // of its own (115230, MAL 40221): the parts under 153406 start at
    // the second season.
    Mock::given(method("GET"))
        .and(path("/anime/43004"))
        .and(query_param("include", "mappings"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mappings(
            "43004",
            Some(40_221),
            Some(115_230),
        )))
        .mount(&kitsu)
        .await;
    kitsu_mapped(&kitsu, "anilist/anime", 153_406, TOWER_OF_GOD_MAPPED).await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 52_635, TOWER_OF_GOD_MAPPED).await;

    let anilist_server = MockServer::start().await;
    // The public id translations: 52635 → 153406; 40221 → 115230;
    // 59989 → nothing.
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
        .and(body_string_contains("\"idMal\":40221"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":{"id":115230}}}"#),
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
    .await
    .expect("the map");
    assert_eq!(map.shape, Shape::SplitParts);
    assert_eq!(map.segments.len(), 1);
    assert_eq!(map.segments[0].entry, 153_406);
    assert_eq!(map.segments[0].entry_total, 26);
    assert_eq!(map.segments[0].lead, 13);
    assert_eq!(
        (map.segments[0].kitsu_from, map.segments[0].kitsu_to),
        (1, 13)
    );
    // A show Kitsu resolves to nothing is keyed without the entry it is
    // filed under, so its map is asked for again daily: the entry's
    // recount reaches no new key.
    assert_eq!(
        ttl_of(&state, &map_key(ProviderKind::AniList, "49244", None)),
        TRACKER_MAP_UNSETTLED_TTL.as_secs()
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
            anilist_base: Some(&kitsu.uri()),
        },
        "5",
    )
    .await
    .expect("the map");
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
              "episodeCount":130,"startDate":"2022-11-10","endDate":"2025-04-23","subtype":"ONA"}}],
            "meta":{"count":1}}"#,
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
    // AniList's id for MAL 52684, and Kitsu's entries mapped to it: the
    // map reads both to learn which Kitsu entries own the MAL entry.
    Mock::given(method("POST"))
        .and(body_string_contains("MediaIdByMal"))
        .and(body_string_contains("52684"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":{"id":185729}}}"#),
        )
        .mount(&anilist_server)
        .await;
    kitsu_mapped(
        &kitsu,
        "anilist/anime",
        185_729,
        r#"{"data":[{"id":"m2","type":"mappings","relationships":{"item":{"data":{"type":"anime","id":"50289"}}}}],
            "included":[{"id":"50289","type":"anime","attributes":{"canonicalTitle":"Shen Yin Wangzuo 3",
              "episodeCount":52,"startDate":"2025-05-01","endDate":"2026-04-23","subtype":"ONA"}}],
            "meta":{"count":1}}"#,
    )
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
    let map = episode_map_for(&ask, "50289", ProviderMediaId(52_684), Some(182))
        .await
        .expect("the map");
    // Kitsu's 50289 also owns 52684 through AniList's id for it, so the
    // two Kitsu entries share the entry as well: both rules file the
    // episodes the same way, and the shared reading names the map.
    assert_eq!(map.shape, Shape::SharedEntry);
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
    let map = episode_map_for(&ask, "46552", ProviderMediaId(52_684), Some(182))
        .await
        .expect("the map");
    assert_eq!(map.shape, Shape::SharedEntry);
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
        r#"{{"data":[{}],"included":[{}],"meta":{{"count":{}}}}}"#,
        data.join(","),
        included.join(","),
        ids.len()
    )
}

#[tokio::test]
async fn kitsu_claims_an_anilist_entry_by_its_own_id_or_by_the_mal_id_it_records() {
    // An entry mapped by the MAL id the AniList entry records owns it
    // only where AniList takes that MAL id back to the entry, as the
    // write's id resolution does: Kitsu's 12, mapped to MAL 40, resolves
    // to AniList 7, not 4.
    let kitsu = MockServer::start().await;
    kitsu_mapped(&kitsu, "anilist/anime", 1, &mapped_body(&["10"])).await;
    kitsu_mapped(&kitsu, "anilist/anime", 2, EMPTY_PAGE).await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 20, &mapped_body(&["11"])).await;
    kitsu_mapped(&kitsu, "anilist/anime", 3, EMPTY_PAGE).await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 30, EMPTY_PAGE).await;
    kitsu_mapped(&kitsu, "anilist/anime", 4, EMPTY_PAGE).await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 40, &mapped_body(&["12"])).await;
    anilist_id_for_mal(&kitsu, 20, 2).await;
    anilist_id_for_mal(&kitsu, 40, 7).await;
    let state = state_with_kitsu(&kitsu.uri());
    let show = |id: u32, id_mal: Option<u32>| TrackerShow {
        id,
        id_mal,
        ..Default::default()
    };
    let base = kitsu.uri();
    let claimed = |s: TrackerShow| {
        let state = state.clone();
        let base = base.clone();
        async move {
            let provider = anilist("http://unused-anilist");
            let tokens = tokens();
            let ask = MapAsk {
                state: &state,
                kind: ProviderKind::AniList,
                provider: &provider,
                tokens: &tokens,
                anilist_base: Some(&base),
            };
            super::kitsu_side::claimed_on_kitsu(&ask, &s)
                .await
                .expect("read")
        }
    };
    assert!(claimed(show(1, None)).await);
    assert!(claimed(show(2, Some(20))).await);
    assert!(!claimed(show(3, Some(30))).await);
    assert!(!claimed(show(3, None)).await);
    assert!(!claimed(show(4, Some(40))).await);
}

/// AniList, served from `server`, giving `media` as its id for MAL's
/// `mal`.
async fn anilist_id_for_mal(server: &MockServer, mal: u32, media: u32) {
    Mock::given(method("POST"))
        .and(body_string_contains("MediaIdByMal"))
        .and(body_string_contains(format!("\"idMal\":{mal}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(format!(r#"{{"data":{{"Media":{{"id":{media}}}}}}}"#)),
        )
        .mount(server)
        .await;
}

/// AniList, served from `server`, recording `mal` as MAL's id for its
/// `media`.
async fn mal_id_for_anilist(server: &MockServer, media: u32, mal: u32) {
    Mock::given(method("POST"))
        .and(body_string_contains("MalIdByMedia"))
        .and(body_string_contains(format!("\"id\":{media}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(format!(r#"{{"data":{{"Media":{{"idMal":{mal}}}}}}}"#)),
        )
        .mount(server)
        .await;
}

#[tokio::test]
async fn kitsu_claims_a_mal_entry_through_an_entry_mapped_only_by_anilist() {
    // Kitsu's 13 maps only to AniList 700, which AniList records as MAL
    // 70: the write's id resolution takes 13 to MAL 70, so 13 owns it.
    // Kitsu's 14 maps to AniList 701 and to MAL 72 as well, and resolves
    // to 72: it does not own MAL 71, which AniList records for 701.
    let kitsu = MockServer::start().await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 70, EMPTY_PAGE).await;
    kitsu_mapped(&kitsu, "anilist/anime", 700, &mapped_body(&["13"])).await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 71, EMPTY_PAGE).await;
    kitsu_mapped(&kitsu, "anilist/anime", 701, &mapped_body(&["14"])).await;
    Mock::given(method("GET"))
        .and(path("/anime/14"))
        .and(query_param("include", "mappings"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mappings(
            "14",
            Some(72),
            Some(701),
        )))
        .mount(&kitsu)
        .await;
    anilist_id_for_mal(&kitsu, 70, 700).await;
    mal_id_for_anilist(&kitsu, 700, 70).await;
    anilist_id_for_mal(&kitsu, 71, 701).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal("http://unused-mal");
    let tokens = tokens();
    let base = kitsu.uri();
    let ask = MapAsk {
        state: &state,
        kind: ProviderKind::MyAnimeList,
        provider: &provider,
        tokens: &tokens,
        anilist_base: Some(&base),
    };
    let show = |id: u32| TrackerShow {
        id,
        ..Default::default()
    };
    assert!(super::kitsu_side::claimed_on_kitsu(&ask, &show(70))
        .await
        .expect("read"));
    assert!(!super::kitsu_side::claimed_on_kitsu(&ask, &show(71))
        .await
        .expect("read"));
    let shared = super::kitsu_side::sharing_on_kitsu(&ask, &show(70))
        .await
        .expect("read");
    assert_eq!(
        shared.iter().map(|k| k.id.as_str()).collect::<Vec<_>>(),
        ["13"]
    );
}

#[tokio::test]
async fn a_mal_chain_whose_sequel_kitsu_owns_through_anilist_is_identity() {
    // MyAnimeList files Kitsu's 24-episode 80 as 800 (12) and its sequel
    // 801 (12), which sum to Kitsu's count. Kitsu has an entry of its
    // own for the sequel, 81, mapped only to AniList 8010, which AniList
    // records as MAL 801: a write for 81 goes to 801, so the chain does
    // not take it.
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "80",
        detail("80", Some(24), "2020-01-05", "2020-06-28"),
        1,
    )
    .await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 801, EMPTY_PAGE).await;
    kitsu_mapped(&kitsu, "anilist/anime", 8010, &mapped_body(&["81"])).await;
    anilist_id_for_mal(&kitsu, 801, 8010).await;
    mal_id_for_anilist(&kitsu, 8010, 801).await;
    let mal_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/800"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"id":800,"num_episodes":12,"status":"finished_airing","media_type":"tv",
                "start_date":"2020-01-05","end_date":"2020-03-22",
                "related_anime":[{"node":{"id":801},"relation_type":"sequel"}]}"#,
        ))
        .mount(&mal_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/801"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"id":801,"num_episodes":12,"status":"finished_airing","media_type":"tv",
                "start_date":"2020-04-05","end_date":"2020-06-28","related_anime":[]}"#,
        ))
        .mount(&mal_server)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal(&mal_server.uri());
    let tokens = tokens();
    let base = kitsu.uri();
    let ask = MapAsk {
        state: &state,
        kind: ProviderKind::MyAnimeList,
        provider: &provider,
        tokens: &tokens,
        anilist_base: Some(&base),
    };
    let map = episode_map_for(&ask, "80", ProviderMediaId(800), Some(12))
        .await
        .expect("the map");
    assert!(matches!(map.shape, Shape::Identity(_)), "{map:?}");
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
    // AniList takes MAL 50 to 5: 411, mapped by it alone, resolves to 5.
    anilist_id_for_mal(&kitsu, 50, 5).await;
    let state = state_with_kitsu(&kitsu.uri());
    let entry = TrackerShow {
        id: 5,
        id_mal: Some(50),
        ..Default::default()
    };
    let provider = anilist("http://unused-anilist");
    let tokens = tokens();
    let base = kitsu.uri();
    let ask = MapAsk {
        state: &state,
        kind: ProviderKind::AniList,
        provider: &provider,
        tokens: &tokens,
        anilist_base: Some(&base),
    };
    let got = super::kitsu_side::sharing_on_kitsu(&ask, &entry)
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
    let base = kitsu.uri();
    let ask = MapAsk {
        state: &state,
        kind: ProviderKind::AniList,
        provider: &provider,
        tokens: &tokens,
        anilist_base: Some(&base),
    };
    let first = episode_map_for(&ask, "186", ProviderMediaId(210), Some(18))
        .await
        .expect("the map");
    assert_eq!(first.shape, Shape::Chain);
    // AniList recounts 210 as 161: the counts agree now, and the old
    // row is not what answers.
    let moved = episode_map_for(&ask, "186", ProviderMediaId(210), Some(161))
        .await
        .expect("the map");
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

#[tokio::test]
async fn an_entry_whose_mapped_kitsu_entries_kitsu_did_not_list_whole_shares_with_none() {
    let kitsu = MockServer::start().await;
    kitsu_mapped(
        &kitsu,
        "myanimelist/anime",
        6,
        r#"{"data":[{"id":"m1","type":"mappings","relationships":{"item":{"data":{"type":"anime","id":"60"}}}}],
            "included":[{"id":"60","type":"anime","attributes":{"episodeCount":12,"startDate":"2020-01-01"}}],
            "meta":{"count":25}}"#,
    )
    .await;
    let state = state_with_kitsu(&kitsu.uri());
    let entry = TrackerShow {
        id: 6,
        ..Default::default()
    };
    let provider = mal("http://unused-mal");
    let tokens = tokens();
    let base = kitsu.uri();
    let ask = MapAsk {
        state: &state,
        kind: ProviderKind::MyAnimeList,
        provider: &provider,
        tokens: &tokens,
        anilist_base: Some(&base),
    };
    let got = super::kitsu_side::sharing_on_kitsu(&ask, &entry)
        .await
        .expect("read");
    // One Kitsu entry the page did not list stands for the rest: no
    // id, no count, so nothing sums with it and the show is not the
    // entry's only one.
    assert_eq!(got, vec![KitsuShow::default()]);
    // It is still claimed: some Kitsu entry maps to it.
    assert!(super::kitsu_side::claimed_on_kitsu(&ask, &entry)
        .await
        .expect("read"));
}

/// A Kitsu episodes page holding `numbers`, each dated by `dated`.
fn episodes_page(numbers: std::ops::RangeInclusive<u32>, dated: &[(u32, &str)]) -> String {
    let rows: Vec<String> = numbers
        .map(|n| {
            let air = dated
                .iter()
                .find(|(d, _)| *d == n)
                .map_or("null".to_string(), |(_, a)| format!("\"{a}\""));
            format!(
                r#"{{"id":"e{n}","type":"episodes","attributes":{{"number":{n},"airdate":{air}}}}}"#
            )
        })
        .collect();
    format!(r#"{{"data":[{}]}}"#, rows.join(","))
}

#[tokio::test]
async fn an_exact_chain_past_kitsu_s_run_is_read_with_kitsu_s_own_air_dates() {
    // Sonic X on AniList: 52 + 26 = 78; Kitsu's dates end 2004-03, its
    // episodes 53 and 78 are dated to the third season's run.
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "1633",
        detail("1633", Some(78), "2003-04-06", "2004-03-28"),
        1,
    )
    .await;
    kitsu_mapped(&kitsu, "anilist/anime", 129_610, EMPTY_PAGE).await;
    // Only the third season's bounds need Kitsu's dates — episodes 53
    // and 78, pages 3 and 4; the first season is inside Kitsu's run and
    // page 1 is never asked for.
    for (offset, numbers, times) in [("0", 1..=20, 0), ("40", 41..=60, 1), ("60", 61..=78, 1)] {
        Mock::given(method("GET"))
            .and(path("/anime/1633/episodes"))
            .and(query_param("page[offset]", offset))
            .respond_with(ResponseTemplate::new(200).set_body_string(episodes_page(
                numbers,
                &[
                    (1, "2003-08-23"),
                    (52, "2005-03-26"),
                    (53, "2005-09-10"),
                    (78, "2006-05-06"),
                ],
            )))
            .expect(times)
            .mount(&kitsu)
            .await;
    }
    let anilist_server = MockServer::start().await;
    anilist_media(
        &anilist_server,
        1816,
        r#"{"data":{"Media":{"id":1816,"idMal":1816,"episodes":52,"status":"FINISHED","format":"TV",
            "startDate":{"year":2003,"month":4,"day":6},"endDate":{"year":2004,"month":3,"day":28},
            "relations":{"edges":[{"relationType":"SEQUEL","node":{"id":129610,"type":"ANIME","format":"TV"}}]},
            "streamingEpisodes":[]}}}"#,
    )
    .await;
    anilist_media(
        &anilist_server,
        129_610,
        r#"{"data":{"Media":{"id":129610,"idMal":null,"episodes":26,"status":"FINISHED","format":"TV",
            "startDate":{"year":2005,"month":9,"day":10},"endDate":{"year":2006,"month":5,"day":6},
            "relations":{"edges":[]},"streamingEpisodes":[]}}}"#,
    )
    .await;
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
    let map = episode_map_for(&ask, "1633", ProviderMediaId(1816), Some(52))
        .await
        .expect("the map");
    assert_eq!(map.shape, Shape::Chain);
    assert_eq!(map.segments[1].entry, 129_610);
    // The map reads Kitsu's own rows: none of these pages has a
    // thumbnail, and reading their dates asks AniList for none — no
    // lookup is made, so none is cached.
    assert_eq!(
        crate::cache::meta_cache_get(&state.cache_pool, "anilist:eps-thumbs:v2:k1633")
            .expect("cache read"),
        None
    );
}

#[tokio::test]
async fn an_air_dates_read_that_fails_fails_the_map_and_is_kept_nowhere() {
    // Sonic X as above, with the episode page that holds episode 53
    // answering 503: the chain rule's dates are unknown, so the map is
    // not built, not kept, and the write fails with the read's error.
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "1633",
        detail("1633", Some(78), "2003-04-06", "2004-03-28"),
        1,
    )
    .await;
    kitsu_mapped(&kitsu, "anilist/anime", 129_610, EMPTY_PAGE).await;
    Mock::given(method("GET"))
        .and(path("/anime/1633/episodes"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&kitsu)
        .await;
    let anilist_server = MockServer::start().await;
    anilist_media(
        &anilist_server,
        1816,
        r#"{"data":{"Media":{"id":1816,"idMal":1816,"episodes":52,"status":"FINISHED","format":"TV",
            "startDate":{"year":2003,"month":4,"day":6},"endDate":{"year":2004,"month":3,"day":28},
            "relations":{"edges":[{"relationType":"SEQUEL","node":{"id":129610,"type":"ANIME","format":"TV"}}]},
            "streamingEpisodes":[]}}}"#,
    )
    .await;
    anilist_media(
        &anilist_server,
        129_610,
        r#"{"data":{"Media":{"id":129610,"idMal":null,"episodes":26,"status":"FINISHED","format":"TV",
            "startDate":{"year":2005,"month":9,"day":10},"endDate":{"year":2006,"month":5,"day":6},
            "relations":{"edges":[]},"streamingEpisodes":[]}}}"#,
    )
    .await;
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
    let err = episode_map_for(&ask, "1633", ProviderMediaId(1816), Some(52))
        .await
        .expect_err("unread air dates are no answer");
    assert!(
        matches!(err, crate::error::AniError::Upstream { status: 503 }),
        "{err:?}"
    );
    assert_eq!(
        meta_cache_get(
            &state.cache_pool,
            &map_key(ProviderKind::AniList, "1633", Some((1816, Some(52))))
        )
        .expect("read"),
        None
    );
}

#[test]
fn a_map_built_before_the_chain_rule_read_kitsu_s_episode_dates_is_never_served() {
    // #264 wrote its maps under v1, with a chain rule that refused every
    // segment outside Kitsu's start and end dates: a row it wrote for
    // Sonic X says identity. The rule that can place that chain reads
    // under a key no v1 row is under.
    let ours = map_key(ProviderKind::AniList, "1633", Some((1816, Some(52))));
    let before = "tracker-map:v1:anilist:k1633:e1816:t52";
    assert_ne!(ours, before);
    assert_eq!(
        ours.replacen("tracker-map:v2:", "tracker-map:v1:", 1),
        before
    );
}

/// A Kitsu `media-relationships` body with several related anime, each
/// `(role, id, episodes, start, end)`.
fn relationships(related: &[(&str, &str, u32, &str, &str)]) -> String {
    let data: Vec<String> = related
        .iter()
        .enumerate()
        .map(|(n, (role, id, ..))| {
            format!(
                r#"{{"id":"r{n}","type":"mediaRelationships","attributes":{{"role":"{role}"}},
                "relationships":{{"destination":{{"data":{{"type":"anime","id":"{id}"}}}}}}}}"#
            )
        })
        .collect();
    let included: Vec<String> = related
        .iter()
        .map(|(_, id, count, start, end)| {
            format!(
                r#"{{"id":"{id}","type":"anime","attributes":{{"canonicalTitle":"Show {id}",
                "episodeCount":{count},"startDate":"{start}","endDate":"{end}","status":"finished","subtype":"TV"}}}}"#
            )
        })
        .collect();
    format!(
        r#"{{"data":[{}],"included":[{}],"meta":{{"count":{}}}}}"#,
        data.join(","),
        included.join(","),
        related.len()
    )
}

#[tokio::test]
async fn a_third_part_with_no_entry_is_filed_after_every_part_before_it_under_the_entry() {
    // Three Kitsu parts of twelve, one after the other. The first two
    // both map to MyAnimeList 900, a 36-episode entry; the third maps to
    // nothing. Filed through its prequels, the third part is the
    // entry's 25th–36th: the parts under the entry start at the first,
    // not at the nearest prequel that resolves.
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "63",
        detail("63", Some(12), "2022-01-01", "2022-03-25"),
        1,
    )
    .await;
    let a = ("61", 12, "2020-01-01", "2020-03-25");
    let b = ("62", 12, "2021-01-01", "2021-03-25");
    let c = ("63", 12, "2022-01-01", "2022-03-25");
    for (id, related) in [
        ("61", vec![("sequel", b.0, b.1, b.2, b.3)]),
        (
            "62",
            vec![
                ("prequel", a.0, a.1, a.2, a.3),
                ("sequel", c.0, c.1, c.2, c.3),
            ],
        ),
        ("63", vec![("prequel", b.0, b.1, b.2, b.3)]),
    ] {
        Mock::given(method("GET"))
            .and(path(format!("/anime/{id}/media-relationships")))
            .respond_with(ResponseTemplate::new(200).set_body_string(relationships(&related)))
            .mount(&kitsu)
            .await;
    }
    for (id, mal) in [("61", Some(900)), ("62", Some(900)), ("63", None)] {
        Mock::given(method("GET"))
            .and(path(format!("/anime/{id}")))
            .and(query_param("include", "mappings"))
            .respond_with(ResponseTemplate::new(200).set_body_string(mappings(id, mal, None)))
            .mount(&kitsu)
            .await;
    }
    kitsu_mapped(
        &kitsu,
        "myanimelist/anime",
        900,
        &mapped_body(&["61", "62"]),
    )
    .await;
    let mal_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/900"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"id":900,"num_episodes":36,"status":"finished_airing","media_type":"tv",
                "start_date":"2020-01-01","end_date":"2022-03-25","related_anime":[]}"#,
        ))
        .mount(&mal_server)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal(&mal_server.uri());
    let map = episode_map_for_unresolved(
        &MapAsk {
            state: &state,
            kind: ProviderKind::MyAnimeList,
            provider: &provider,
            tokens: &tokens(),
            anilist_base: Some(&kitsu.uri()),
        },
        "63",
    )
    .await
    .expect("the map");
    assert_eq!(map.shape, Shape::SplitParts);
    assert_eq!(map.segments.len(), 1);
    assert_eq!(map.segments[0].entry, 900);
    assert_eq!(map.segments[0].entry_total, 36);
    assert_eq!(map.segments[0].lead, 24);
    assert_eq!(
        (map.segments[0].kitsu_from, map.segments[0].kitsu_to),
        (1, 12)
    );
}

#[tokio::test]
async fn the_last_of_five_parts_under_one_entry_is_filed_after_the_four_before_it() {
    // Five Kitsu parts of twelve, one after the other, all mapped to
    // MyAnimeList 901, a 60-episode entry. Read from the fifth, the
    // walk back reaches the first part, four prequels up, and the fifth
    // is the entry's 49th–60th.
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "85",
        detail("85", Some(12), "2024-01-01", "2024-03-25"),
        1,
    )
    .await;
    let parts: Vec<(String, String, String)> = (0..5)
        .map(|i| {
            (
                format!("8{}", i + 1),
                format!("{}-01-01", 2020 + i),
                format!("{}-03-25", 2020 + i),
            )
        })
        .collect();
    for (i, (id, ..)) in parts.iter().enumerate() {
        let mut related = Vec::new();
        if i > 0 {
            let (p, s, e) = &parts[i - 1];
            related.push(("prequel", p.as_str(), 12, s.as_str(), e.as_str()));
        }
        if let Some((n, s, e)) = parts.get(i + 1) {
            related.push(("sequel", n.as_str(), 12, s.as_str(), e.as_str()));
        }
        Mock::given(method("GET"))
            .and(path(format!("/anime/{id}/media-relationships")))
            .respond_with(ResponseTemplate::new(200).set_body_string(relationships(&related)))
            .mount(&kitsu)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/anime/{id}")))
            .and(query_param("include", "mappings"))
            .respond_with(ResponseTemplate::new(200).set_body_string(mappings(id, Some(901), None)))
            .mount(&kitsu)
            .await;
    }
    kitsu_mapped(
        &kitsu,
        "myanimelist/anime",
        901,
        &mapped_body(&["81", "82", "83", "84", "85"]),
    )
    .await;
    let mal_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/901"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"id":901,"num_episodes":60,"status":"finished_airing","media_type":"tv",
                "start_date":"2020-01-01","end_date":"2024-03-25","related_anime":[]}"#,
        ))
        .mount(&mal_server)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal(&mal_server.uri());
    let map = episode_map_for(
        &MapAsk {
            state: &state,
            kind: ProviderKind::MyAnimeList,
            provider: &provider,
            tokens: &tokens(),
            anilist_base: Some(&kitsu.uri()),
        },
        "85",
        ProviderMediaId(901),
        Some(60),
    )
    .await
    .expect("the map");
    assert_eq!(map.shape, Shape::SplitParts);
    assert_eq!(map.segments.len(), 1);
    assert_eq!(map.segments[0].entry_total, 60);
    assert_eq!(map.segments[0].lead, 48);
}

/// Mount a Kitsu chain of twelve-episode parts, a year apart from
/// 2020, each the single sequel of the one before; `mal` is each
/// part's MyAnimeList mapping, if any.
async fn mount_chain(kitsu: &MockServer, chain: &[(&str, Option<u32>)]) {
    let chain: Vec<(&str, Option<u32>, Option<u32>)> =
        chain.iter().map(|(id, mal)| (*id, *mal, None)).collect();
    mount_chain_mapped(kitsu, &chain).await;
}

/// [`mount_chain`] with each part's MyAnimeList and AniList mappings.
async fn mount_chain_mapped(kitsu: &MockServer, chain: &[(&str, Option<u32>, Option<u32>)]) {
    let dated = |i: usize| (format!("{}-01-01", 2020 + i), format!("{}-03-25", 2020 + i));
    for (i, (id, mal, anilist)) in chain.iter().enumerate() {
        let mut related = Vec::new();
        let before = i.checked_sub(1).map(|j| (chain[j].0, dated(j)));
        let after = chain.get(i + 1).map(|(n, ..)| (*n, dated(i + 1)));
        if let Some((p, (s, e))) = &before {
            related.push(("prequel", *p, 12, s.clone(), e.clone()));
        }
        if let Some((n, (s, e))) = &after {
            related.push(("sequel", *n, 12, s.clone(), e.clone()));
        }
        let related: Vec<(&str, &str, u32, &str, &str)> = related
            .iter()
            .map(|(r, id, n, s, e)| (*r, *id, *n, s.as_str(), e.as_str()))
            .collect();
        Mock::given(method("GET"))
            .and(path(format!("/anime/{id}/media-relationships")))
            .respond_with(ResponseTemplate::new(200).set_body_string(relationships(&related)))
            .mount(kitsu)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/anime/{id}")))
            .and(query_param("include", "mappings"))
            .respond_with(ResponseTemplate::new(200).set_body_string(mappings(id, *mal, *anilist)))
            .mount(kitsu)
            .await;
    }
}

/// A MyAnimeList server answering for one finished entry.
async fn mal_entry(id: u32, total: u32) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/anime/{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!(
            r#"{{"id":{id},"num_episodes":{total},"status":"finished_airing","media_type":"tv",
                "start_date":"2020-01-01","end_date":"2030-03-25","related_anime":[]}}"#
        )))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn a_part_with_no_entry_between_two_under_the_entry_leaves_the_first_in_the_count() {
    // Four parts of twelve: the first, third and fourth map to
    // MyAnimeList 902, a 48-episode entry; the second maps to nothing.
    // Read from the fourth, the parts under the entry run from the
    // first, through the second, which has no entry of its own: the
    // fourth is the entry's 37th–48th.
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "94",
        detail("94", Some(12), "2023-01-01", "2023-03-25"),
        1,
    )
    .await;
    mount_chain(
        &kitsu,
        &[
            ("91", Some(902)),
            ("92", None),
            ("93", Some(902)),
            ("94", Some(902)),
        ],
    )
    .await;
    kitsu_mapped(
        &kitsu,
        "myanimelist/anime",
        902,
        &mapped_body(&["91", "93", "94"]),
    )
    .await;
    let mal_server = mal_entry(902, 48).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal(&mal_server.uri());
    let map = episode_map_for(
        &MapAsk {
            state: &state,
            kind: ProviderKind::MyAnimeList,
            provider: &provider,
            tokens: &tokens(),
            anilist_base: Some(&kitsu.uri()),
        },
        "94",
        ProviderMediaId(902),
        Some(48),
    )
    .await
    .expect("the map");
    assert_eq!(map.shape, Shape::SplitParts);
    assert_eq!(map.segments[0].entry_total, 48);
    assert_eq!(map.segments[0].lead, 36);
}
#[tokio::test]
async fn a_part_four_unresolved_prequels_after_its_entry_is_filed_under_it() {
    // Six parts of twelve: only the first maps to MyAnimeList 903, a
    // 72-episode entry. The sixth, mapped to nothing, is found through
    // four prequels with no entry of their own and a fifth that has
    // one: it is the entry's 61st–72nd.
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "76",
        detail("76", Some(12), "2025-01-01", "2025-03-25"),
        1,
    )
    .await;
    mount_chain(
        &kitsu,
        &[
            ("71", Some(903)),
            ("72", None),
            ("73", None),
            ("74", None),
            ("75", None),
            ("76", None),
        ],
    )
    .await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 903, &mapped_body(&["71"])).await;
    let mal_server = mal_entry(903, 72).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal(&mal_server.uri());
    let map = episode_map_for_unresolved(
        &MapAsk {
            state: &state,
            kind: ProviderKind::MyAnimeList,
            provider: &provider,
            tokens: &tokens(),
            anilist_base: Some(&kitsu.uri()),
        },
        "76",
    )
    .await
    .expect("the map");
    assert_eq!(map.shape, Shape::SplitParts);
    assert_eq!(map.segments[0].entry_total, 72);
    assert_eq!(map.segments[0].lead, 60);
}

#[tokio::test]
async fn a_chain_with_a_part_mapped_to_the_entry_past_its_total_is_no_split() {
    // Three parts of twelve, one after the other, all mapped to
    // MyAnimeList 904, a 24-episode entry. The first two reach the
    // entry's total, but the third is mapped to it too: the parts are
    // not the whole of the entry, and the second part's finale must not
    // complete it.
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "52",
        detail("52", Some(12), "2021-01-01", "2021-03-25"),
        1,
    )
    .await;
    mount_chain(
        &kitsu,
        &[("51", Some(904)), ("52", Some(904)), ("53", Some(904))],
    )
    .await;
    kitsu_mapped(
        &kitsu,
        "myanimelist/anime",
        904,
        &mapped_body(&["51", "52", "53"]),
    )
    .await;
    let mal_server = mal_entry(904, 24).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal(&mal_server.uri());
    let map = episode_map_for(
        &MapAsk {
            state: &state,
            kind: ProviderKind::MyAnimeList,
            provider: &provider,
            tokens: &tokens(),
            anilist_base: Some(&kitsu.uri()),
        },
        "52",
        ProviderMediaId(904),
        Some(24),
    )
    .await
    .expect("the map");
    assert_eq!(map, EpisodeMap::identity(Identity::Unexplained));
}

#[tokio::test]
async fn a_part_past_the_total_mapped_only_through_the_other_tracker_still_contradicts_the_chain() {
    // Three parts of twelve against MyAnimeList 905, a 24-episode entry.
    // The first two map to it; the third maps only to AniList 5300, which
    // AniList records as MAL 905. Kitsu's page of the entries mapped to
    // 905 lists the first two alone; the third is found mapped to the
    // entry through AniList's id for 905, and by the walk past the
    // total. The second part's finale must not complete it.
    let kitsu = MockServer::start().await;
    kitsu_detail(
        &kitsu,
        "42",
        detail("42", Some(12), "2021-01-01", "2021-03-25"),
        1,
    )
    .await;
    mount_chain_mapped(
        &kitsu,
        &[
            ("41", Some(905), None),
            ("42", Some(905), None),
            ("43", None, Some(5_300)),
        ],
    )
    .await;
    kitsu_mapped(
        &kitsu,
        "myanimelist/anime",
        905,
        &mapped_body(&["41", "42"]),
    )
    .await;
    let anilist_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains("MalIdByMedia"))
        .and(body_string_contains("5300"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":{"idMal":905}}}"#),
        )
        .mount(&anilist_server)
        .await;
    // AniList's id for MAL 905, and Kitsu's page of the entries mapped
    // to it, which lists the third part.
    Mock::given(method("POST"))
        .and(body_string_contains("MediaIdByMal"))
        .and(body_string_contains("905"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":{"id":5300}}}"#),
        )
        .mount(&anilist_server)
        .await;
    kitsu_mapped(&kitsu, "anilist/anime", 5_300, &mapped_body(&["43"])).await;
    let mal_server = mal_entry(905, 24).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal(&mal_server.uri());
    let anilist_uri = anilist_server.uri();
    let map = episode_map_for(
        &MapAsk {
            state: &state,
            kind: ProviderKind::MyAnimeList,
            provider: &provider,
            tokens: &tokens(),
            anilist_base: Some(&anilist_uri),
        },
        "42",
        ProviderMediaId(905),
        Some(24),
    )
    .await
    .expect("the map");
    assert_eq!(map, EpisodeMap::identity(Identity::Unexplained));
}
