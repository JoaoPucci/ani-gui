//! Tests for `crate::commands::account`. Extracted via `#[path]` so
//! the dispatcher + helper complexity doesn't pile onto `account.rs`'s
//! CCN budget — per `project_crap_inline_test_gotcha`.

use super::*;
use crate::account::pkce::PkceMethod;

/// Build an `AppState` whose Kitsu client points at `kitsu_uri` (a
/// wiremock server) so id-resolution tests can mock the mappings
/// endpoint. Everything else is throwaway.
#[cfg(test)]
fn state_with_kitsu(kitsu_uri: &str) -> std::sync::Arc<crate::app::AppState> {
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    Arc::new(crate::app::AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        ani_cli_path: PathBuf::from("/tmp/ani-cli"),
        bash_path: None,
        bundled_bin: None,
        history_path: PathBuf::from("/tmp/ani-cli/ani-hsts"),
        scraper_slots: Arc::new(Semaphore::new(crate::app::SCRAPER_CONCURRENCY)),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: AccountWriteLocks::new(),
    })
}

/// Kitsu `/anime/:id?include=mappings` body carrying a MAL mapping.
#[cfg(test)]
const KITSU_MAL_MAPPING_BODY: &str = r#"{
    "data": { "id": "12", "type": "anime", "attributes": { "canonicalTitle": "One Piece" } },
    "included": [{
        "id": "1175",
        "type": "mappings",
        "attributes": { "externalSite": "myanimelist/anime", "externalId": "21" }
    }]
}"#;

#[tokio::test]
async fn resolve_native_media_id_mal_is_the_mapped_mal_id() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAL_MAPPING_BODY))
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::MyAnimeList, "12", None)
        .await
        .expect("resolve ok");
    assert_eq!(got, Some(ProviderMediaId(21)));
}

#[tokio::test]
async fn resolve_native_media_id_anilist_bridges_mal_to_media_id() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAL_MAPPING_BODY))
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"Media":{"id":154587}}}"#),
        )
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::AniList, "12", Some(&anilist.uri()))
        .await
        .expect("resolve ok");
    assert_eq!(got, Some(ProviderMediaId(154587)));
}

#[tokio::test]
async fn resolve_native_media_id_none_when_kitsu_has_no_mal_mapping() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/999"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"id":"999","type":"anime"},"included":[]}"#),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::MyAnimeList, "999", None)
        .await
        .expect("resolve ok");
    assert_eq!(got, None);
}

#[tokio::test]
async fn resolve_native_media_id_none_when_anilist_unmapped() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAL_MAPPING_BODY))
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#),
        )
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::AniList, "12", Some(&anilist.uri()))
        .await
        .expect("resolve ok");
    assert_eq!(got, None);
}

/// Yani Neko's real mapping shape (Kitsu 50551): only anilist/anime,
/// no MAL mapping yet. The MAL-pivot resolver returned None for every
/// provider here, so fresh seasonal shows couldn't be written to ANY
/// tracker even though the ids to reach both existed.
#[cfg(test)]
const KITSU_ANILIST_ONLY_MAPPING_BODY: &str = r#"{
    "data": { "id": "50551", "type": "anime", "attributes": { "canonicalTitle": "Yani Neko" } },
    "included": [{
        "id": "1",
        "type": "mappings",
        "attributes": { "externalSite": "anilist/anime", "externalId": "207141" }
    }]
}"#;

/// One Piece with BOTH mappings — the fixture for "primary path still
/// wins / fallback only fires when the bridge comes up empty".
#[cfg(test)]
const KITSU_BOTH_MAPPINGS_BODY: &str = r#"{
    "data": { "id": "12", "type": "anime", "attributes": { "canonicalTitle": "One Piece" } },
    "included": [
        { "id": "1", "type": "mappings", "attributes": { "externalSite": "myanimelist/anime", "externalId": "21" } },
        { "id": "2", "type": "mappings", "attributes": { "externalSite": "anilist/anime", "externalId": "207141" } }
    ]
}"#;

#[tokio::test]
async fn resolve_anilist_uses_direct_kitsu_mapping_when_mal_mapping_absent() {
    // No AniList GraphQL mock mounted on purpose: the direct
    // anilist/anime mapping must be enough — no bridge round-trip.
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/50551"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_ANILIST_ONLY_MAPPING_BODY),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::AniList, "50551", None)
        .await
        .expect("resolve ok");
    assert_eq!(got, Some(ProviderMediaId(207141)));
}

#[tokio::test]
async fn resolve_mal_bridges_anilist_mapping_to_idmal_when_mal_mapping_absent() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/50551"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_ANILIST_ONLY_MAPPING_BODY),
        )
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"Media":{"idMal":63403}}}"#),
        )
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(
        &state,
        ProviderKind::MyAnimeList,
        "50551",
        Some(&anilist.uri()),
    )
    .await
    .expect("resolve ok");
    assert_eq!(got, Some(ProviderMediaId(63403)));
}

#[tokio::test]
async fn resolve_anilist_falls_back_to_direct_mapping_when_bridge_unindexed() {
    // MAL mapping present but AniList doesn't index that MAL id
    // (Media:null from the bridge query) — the direct anilist/anime
    // mapping must still win over giving up.
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_BOTH_MAPPINGS_BODY),
        )
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#),
        )
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::AniList, "12", Some(&anilist.uri()))
        .await
        .expect("resolve ok");
    assert_eq!(got, Some(ProviderMediaId(207141)));
}

/// Kitsu `/mappings?filter[externalSite]=anilist/anime` hit for Yani
/// Neko (Kitsu 50551 ← AniList 207141) — the Watch-Later bridge's
/// fallback target when the MAL-id lookup comes up empty.
#[cfg(test)]
const KITSU_ANILIST_MAPPINGS_HIT_BODY: &str = r#"{
    "data": [{
        "id": "9001",
        "type": "mappings",
        "attributes": { "externalSite": "anilist/anime", "externalId": "207141" },
        "relationships": { "item": { "data": { "type": "anime", "id": "50551" } } }
    }],
    "included": [{
        "id": "50551",
        "type": "anime",
        "attributes": {
            "canonicalTitle": "Yani Neko",
            "titles": { "en": "Chainsmoker Cat" },
            "slug": "yani-neko",
            "synopsis": "Catgirl with a smoking habit.",
            "startDate": "2026-07-03",
            "endDate": null,
            "episodeCount": 12,
            "averageRating": null,
            "subtype": "TV",
            "status": "current",
            "ageRating": "R",
            "popularityRank": 9000,
            "posterImage": null,
            "coverImage": null
        }
    }]
}"#;

#[cfg(test)]
const KITSU_MAPPINGS_EMPTY_BODY: &str = r#"{ "data": [], "included": [] }"#;

#[tokio::test]
async fn watch_later_bridge_falls_back_to_anilist_mapping_for_unmapped_mal_id() {
    // Kitsu can't answer for MAL 63403 (no myanimelist/anime mapping),
    // but AniList knows Media(idMal: 63403) = 207141 and Kitsu carries
    // the anilist/anime mapping for it — the bridged card must render.
    use wiremock::matchers::{method, path, query_param};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalSite]", "myanimelist/anime"))
        .and(query_param("filter[externalId]", "63403"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAPPINGS_EMPTY_BODY),
        )
        .mount(&kitsu)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalSite]", "anilist/anime"))
        .and(query_param("filter[externalId]", "207141"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_ANILIST_MAPPINGS_HIT_BODY),
        )
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    // Wire shape is the batched Page(idMal_in:) query — one request
    // covers every Kitsu-missing id in the load (Codex P2 #3565216298),
    // replacing the per-id Media(idMal:) call this mock originally
    // spoke. The behavior under assertion (the card bridges) is
    // unchanged.
    wiremock::Mock::given(method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"Page":{"media":[{"id":207141,"idMal":63403}]}}}"#),
        )
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let refs = kitsu_for_mal_ids_with_anilist_base(&state, vec![63403], Some(&anilist.uri())).await;
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].id, "50551");
    assert_eq!(refs[0].canonical_title, "Yani Neko");
}

#[tokio::test]
async fn watch_later_bridge_drops_id_when_anilist_does_not_know_it_either() {
    use wiremock::matchers::method;
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAPPINGS_EMPTY_BODY),
        )
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    // Batched wire shape (see the fallback test above): an id AniList
    // doesn't index is simply absent from the Page result.
    wiremock::Mock::given(method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"Page":{"media":[]}}}"#),
        )
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let refs =
        kitsu_for_mal_ids_with_anilist_base(&state, vec![99_999_999], Some(&anilist.uri())).await;
    assert!(refs.is_empty());
}

/// Second anilist-mapping hit (Kitsu 50552 ← AniList 207142) so the
/// batching test can bridge two misses.
#[cfg(test)]
const KITSU_ANILIST_MAPPINGS_HIT_BODY_2: &str = r#"{
    "data": [{
        "id": "9002",
        "type": "mappings",
        "attributes": { "externalSite": "anilist/anime", "externalId": "207142" },
        "relationships": { "item": { "data": { "type": "anime", "id": "50552" } } }
    }],
    "included": [{
        "id": "50552",
        "type": "anime",
        "attributes": {
            "canonicalTitle": "Neko to Ryuu",
            "titles": {},
            "slug": "neko-to-ryuu",
            "synopsis": null,
            "startDate": "2026-07-04",
            "endDate": null,
            "episodeCount": 12,
            "averageRating": null,
            "subtype": "TV",
            "status": "current",
            "ageRating": "PG",
            "popularityRank": 9001,
            "posterImage": null,
            "coverImage": null
        }
    }]
}"#;

#[tokio::test]
async fn watch_later_bridge_batches_anilist_lookups_into_one_request() {
    // Two Kitsu-unmapped MAL ids must cost exactly ONE AniList call
    // (the batched Page(idMal_in:) query), not one per miss — a rail
    // load full of fresh seasonal titles would otherwise blow through
    // AniList's 30 req/min budget (Codex P2 #3565216298).
    use wiremock::matchers::{method, path, query_param};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalSite]", "myanimelist/anime"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAPPINGS_EMPTY_BODY),
        )
        .mount(&kitsu)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalSite]", "anilist/anime"))
        .and(query_param("filter[externalId]", "207141"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_ANILIST_MAPPINGS_HIT_BODY),
        )
        .mount(&kitsu)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalSite]", "anilist/anime"))
        .and(query_param("filter[externalId]", "207142"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_ANILIST_MAPPINGS_HIT_BODY_2),
        )
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Page":{"media":[
                {"id":207141,"idMal":63403},
                {"id":207142,"idMal":63404}
            ]}}}"#,
        ))
        .expect(1)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let refs =
        kitsu_for_mal_ids_with_anilist_base(&state, vec![63403, 63404], Some(&anilist.uri())).await;
    assert_eq!(refs.len(), 2);
    // Input order is preserved across the two-phase fill.
    assert_eq!(refs[0].id, "50551");
    assert_eq!(refs[1].id, "50552");
    // MockServer verifies the .expect(1) on drop.
}

#[tokio::test]
async fn watch_later_bridge_direct_mal_hit_never_touches_anilist() {
    // anilist_base points at an unroutable address — if the fallback
    // fired on a direct hit, the entry would still survive (failures
    // are swallowed), so assert via wiremock that zero AniList calls
    // happen at all.
    use wiremock::matchers::{method, path, query_param};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalSite]", "myanimelist/anime"))
        .and(query_param("filter[externalId]", "21"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{
                    "data": [{
                        "id": "1175",
                        "type": "mappings",
                        "attributes": { "externalSite": "myanimelist/anime", "externalId": "21" },
                        "relationships": { "item": { "data": { "type": "anime", "id": "12" } } }
                    }],
                    "included": [{
                        "id": "12",
                        "type": "anime",
                        "attributes": {
                            "canonicalTitle": "One Piece",
                            "titles": {},
                            "slug": "one-piece",
                            "synopsis": null,
                            "startDate": "1999-10-20",
                            "endDate": null,
                            "episodeCount": null,
                            "averageRating": null,
                            "subtype": "TV",
                            "status": "current",
                            "ageRating": "PG",
                            "popularityRank": 25,
                            "posterImage": null,
                            "coverImage": null
                        }
                    }]
                }"#,
        ))
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("{}"))
        .expect(0)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let refs = kitsu_for_mal_ids_with_anilist_base(&state, vec![21], Some(&anilist.uri())).await;
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].id, "12");
    // MockServer verifies the .expect(0) on drop.
}

#[tokio::test]
async fn resolve_none_when_kitsu_carries_no_tracker_mappings_at_all() {
    // Belt-and-suspenders for the fallback era: with neither mapping,
    // both providers resolve to None without any AniList round-trip.
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/777"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"id":"777","type":"anime"},"included":[]}"#),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    for kind in [ProviderKind::MyAnimeList, ProviderKind::AniList] {
        let got = resolve_native_media_id(&state, kind, "777", None)
            .await
            .expect("resolve ok");
        assert_eq!(got, None);
    }
}

#[tokio::test]
async fn push_progress_skips_unmappable_show_without_writing() {
    // A show Kitsu can't map to MAL → resolve yields None → push_progress
    // returns Ok(None) and never reaches update_entry. The provider
    // built by the dispatcher hits the real MAL host, so a stray write
    // attempt would surface as Network/Upstream, not Ok(None) — proving
    // the short-circuit fires before any upstream call.
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/999"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"id":"999","type":"anime"},"included":[]}"#),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let tokens = Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    let got = push_progress(
        &state,
        ProviderKind::MyAnimeList,
        &tokens,
        "999",
        crate::account::provider::EntryUpdate {
            progress_episodes: Some(5),
            ..Default::default()
        },
    )
    .await
    .expect("push ok");
    assert!(got.is_none(), "unmappable show must short-circuit to None");
}

/// A MAL provider pointed at a wiremock server.
#[cfg(test)]
fn mal_provider(api_uri: &str) -> crate::meta::mal_user::MalProvider {
    crate::meta::mal_user::MalProvider::with_bases(
        reqwest::Client::new(),
        api_uri.to_string(),
        "http://unused-token".to_string(),
        crate::meta::mal_user::MalRefreshState::new(),
    )
}

#[cfg(test)]
fn test_tokens() -> Tokens {
    Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

#[tokio::test]
async fn push_progress_via_folds_the_cache_write_through_under_the_lock() {
    // Codex P2 #3423108941 / #3423044438: the mark-watched cache
    // write-through must run under push_progress's per-show lock (not
    // deferred to the route afterwards), so a stale write can't land
    // after an explicit edit. push_progress_via writes the cache itself;
    // assert the row lands. Mock current_entry (reconcile reads it),
    // update_entry, and /users/@me (cache owner).
    use crate::account::cache::list_entries;
    use wiremock::matchers::{method, path};
    let mal = wiremock::MockServer::start().await;
    // Not yet on the list → reconcile treats it as new and advances.
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/21"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(r#"{"id":21}"#))
        .mount(&mal)
        .await;
    wiremock::Mock::given(method("PATCH"))
        .and(path("/anime/21/my_list_status"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"status":"watching","num_episodes_watched":5,"is_rewatching":false}"#,
        ))
        .mount(&mal)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/users/@me"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(r#"{"id":4242,"name":"s"}"#),
        )
        .mount(&mal)
        .await;
    let state = state_with_kitsu("http://unused-kitsu");
    let provider = mal_provider(&mal.uri());
    let entry = push_progress_via(
        &state,
        ProviderKind::MyAnimeList,
        &provider,
        &test_tokens(),
        ProviderMediaId(21),
        crate::account::provider::EntryUpdate {
            progress_episodes: Some(5),
            ..Default::default()
        },
    )
    .await
    .expect("push ok")
    .expect("mapped + written");
    assert_eq!(entry.progress_episodes, 5);
    let cached = list_entries(&state.cache_pool, ProviderKind::MyAnimeList, "4242").unwrap();
    assert_eq!(
        cached.len(),
        1,
        "the mark-watched write-through landed in the cache under the lock"
    );
    assert_eq!(cached[0].progress_episodes, 5);
}

#[test]
fn account_write_locks_share_one_mutex_per_show() {
    // Codex P2 #3387237642 / #3428252253: serialization only works if
    // every call for the same (provider, show) gets the SAME mutex, and
    // distinct shows get distinct ones. The key is the Kitsu id so the
    // lock can be taken BEFORE native-id resolution — the editor's seed
    // read and a mark-watched write serialize over the whole resolve →
    // read/write sequence, not just the post-resolve tail.
    let locks = AccountWriteLocks::new();
    let a1 = locks.for_show(ProviderKind::MyAnimeList, "12");
    let a2 = locks.for_show(ProviderKind::MyAnimeList, "12");
    assert!(std::sync::Arc::ptr_eq(&a1, &a2), "same show → same mutex");
    let other_show = locks.for_show(ProviderKind::MyAnimeList, "99");
    assert!(
        !std::sync::Arc::ptr_eq(&a1, &other_show),
        "different show → different mutex"
    );
    let other_provider = locks.for_show(ProviderKind::AniList, "12");
    assert!(
        !std::sync::Arc::ptr_eq(&a1, &other_provider),
        "same id, different provider → different mutex"
    );
}

#[tokio::test]
async fn push_progress_waits_for_the_show_lock_before_resolving() {
    // Codex P2 #3428252253: the mark-watched write must take the per-show
    // lock BEFORE resolve_native_media_id, keyed on the Kitsu id, so it
    // serializes with the editor's seed read (same lock) across the
    // resolve window. Hold the Kitsu-id lock and assert push_progress
    // can't begin (blocks at the lock, never reaching resolve) until it's
    // released.
    use std::time::Duration;
    let state = state_with_kitsu("http://127.0.0.1:1");
    let tokens = Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    let show_lock = state
        .account_write_locks
        .for_show(ProviderKind::MyAnimeList, "12");
    let guard = show_lock.lock().await;
    let update = crate::account::provider::EntryUpdate {
        progress_episodes: Some(5),
        ..Default::default()
    };
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_millis(200)) => {}
        _ = push_progress(&state, ProviderKind::MyAnimeList, &tokens, "12", update) => {
            panic!("push_progress proceeded while the show's Kitsu-id lock was held");
        }
    }
    drop(guard);
}

#[test]
fn reconcile_monotonic_clamps_progress_and_reconciles_status() {
    use crate::account::provider::{CurrentEntry, EntryUpdate};
    // The fan-out sends progress-only for non-finale, Completed at the
    // finale. Helpers mirror that.
    let progress_only = |ep| EntryUpdate {
        progress_episodes: Some(ep),
        ..Default::default()
    };
    let watching = |ep| EntryUpdate {
        status: Some(ListStatus::Watching),
        progress_episodes: Some(ep),
        ..Default::default()
    };
    let entry = |status, ep| {
        Some(CurrentEntry {
            status,
            progress_episodes: ep,
        })
    };

    // Codex P2 #3387383171: a progress write to a not-yet-listed show
    // creates it as Watching.
    assert_eq!(
        reconcile_monotonic(progress_only(1), None),
        Some(watching(1))
    );
    // …and promotes a Plan-to-Watch row out of planning.
    assert_eq!(
        reconcile_monotonic(progress_only(6), entry(ListStatus::Planning, 0)),
        Some(watching(6))
    );

    // Codex P2 #3387568872: a planning row already at the same/higher
    // count still promotes to Watching (status-only) — the progress is
    // dropped as non-advancing but the title must leave Watch Later.
    assert_eq!(
        reconcile_monotonic(progress_only(3), entry(ListStatus::Planning, 10)),
        Some(EntryUpdate {
            status: Some(ListStatus::Watching),
            ..Default::default()
        })
    );

    // Codex P2 #3387319861: an advancing write must NOT touch a
    // rewatching (or already-watching) row's status — progress only.
    assert_eq!(
        reconcile_monotonic(progress_only(6), entry(ListStatus::Rewatching, 5)),
        Some(progress_only(6))
    );
    assert_eq!(
        reconcile_monotonic(progress_only(6), entry(ListStatus::Watching, 5)),
        Some(progress_only(6))
    );

    // Codex P1 #3386909281: a non-advancing progress write is dropped
    // entirely — never regress.
    assert_eq!(
        reconcile_monotonic(progress_only(3), entry(ListStatus::Watching, 10)),
        None
    );
    assert_eq!(
        reconcile_monotonic(progress_only(10), entry(ListStatus::Watching, 10)),
        None
    );

    // Codex P2 #3387051891: a Completed write at unchanged progress is
    // still needed — keep the status, drop only the non-advancing
    // progress field.
    let finale = EntryUpdate {
        status: Some(ListStatus::Completed),
        progress_episodes: Some(12),
        ..Default::default()
    };
    assert_eq!(
        reconcile_monotonic(finale, entry(ListStatus::Watching, 12)),
        Some(EntryUpdate {
            status: Some(ListStatus::Completed),
            progress_episodes: None,
            ..Default::default()
        })
    );

    // A score-only edit at unchanged progress survives (not a no-op,
    // and no spurious promotion since progress was dropped).
    let rescore = EntryUpdate {
        progress_episodes: Some(5),
        score_0_to_100: Some(90),
        ..Default::default()
    };
    assert_eq!(
        reconcile_monotonic(rescore, entry(ListStatus::Watching, 10)),
        Some(EntryUpdate {
            score_0_to_100: Some(90),
            ..Default::default()
        })
    );
}

#[test]
fn reconcile_monotonic_preserves_rewatching_at_finale() {
    use crate::account::provider::{CurrentEntry, EntryUpdate};
    // Codex P2 #3415780486: the fan-out sends Completed when the user
    // finishes a finished series. If the tracker row is already
    // rewatching/repeating, completing it would clear AniList REPEATING
    // / MAL is_rewatching — contradicting the preserve-rewatching rule.
    // The finale Completed must be dropped for a rewatching row.
    let finale = |ep| EntryUpdate {
        status: Some(ListStatus::Completed),
        progress_episodes: Some(ep),
        ..Default::default()
    };
    let entry = |status, ep| {
        Some(CurrentEntry {
            status,
            progress_episodes: ep,
        })
    };

    // Rewatcher advancing into the finale: progress still flows, but the
    // Completed status is stripped so the row stays rewatching.
    assert_eq!(
        reconcile_monotonic(finale(12), entry(ListStatus::Rewatching, 11)),
        Some(EntryUpdate {
            progress_episodes: Some(12),
            ..Default::default()
        })
    );
    // Rewatcher already at the cap (re-finishing): nothing actionable —
    // progress non-advancing and the Completed is dropped → skip.
    assert_eq!(
        reconcile_monotonic(finale(12), entry(ListStatus::Rewatching, 12)),
        None
    );
    // A genuine Watching → Completed finale is still honored.
    assert_eq!(
        reconcile_monotonic(finale(12), entry(ListStatus::Watching, 11)),
        Some(finale(12))
    );
}

#[test]
fn build_entry_update_rejects_empty_and_unknown_status() {
    // Codex P2 #3381617932: an all-absent update, or a status typo
    // that silently parses to None, would still call update_entry —
    // and since both providers upsert, that creates a list row with
    // upstream defaults. Reject both so a malformed fan-out request
    // is a no-op error, not a phantom "watching" entry.
    assert!(
        build_entry_update(None, None, None).is_err(),
        "all-absent update must be rejected"
    );
    assert!(
        build_entry_update(Some("not_a_status"), Some(5), None).is_err(),
        "unrecognized status must be rejected, not dropped to None"
    );
    let ok = build_entry_update(Some("watching"), Some(5), None).expect("valid update");
    assert_eq!(ok.status, Some(ListStatus::Watching));
    assert_eq!(ok.progress_episodes, Some(5));
    // Progress-only (no status) is a legitimate update.
    let progress_only = build_entry_update(None, Some(7), None).expect("progress-only ok");
    assert!(progress_only.status.is_none());
    assert_eq!(progress_only.progress_episodes, Some(7));
}

#[test]
fn provider_for_kind_dispatches_anilist_and_mal_but_not_inhouse() {
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    let state = Arc::new(crate::app::AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        ani_cli_path: PathBuf::from("/tmp/ani-cli"),
        bash_path: None,
        bundled_bin: None,
        history_path: PathBuf::from("/tmp/ani-cli/ani-hsts"),
        scraper_slots: Arc::new(Semaphore::new(crate::app::SCRAPER_CONCURRENCY)),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: AccountWriteLocks::new(),
    });
    assert!(provider_for_kind(&state, ProviderKind::AniList).is_some());
    assert!(provider_for_kind(&state, ProviderKind::MyAnimeList).is_some());
    assert!(provider_for_kind(&state, ProviderKind::InHouse).is_none());
}

#[test]
fn pkce_for_kind_picks_method_per_provider() {
    assert_eq!(
        pkce_for_kind(ProviderKind::AniList).method,
        PkceMethod::S256
    );
    assert_eq!(
        pkce_for_kind(ProviderKind::MyAnimeList).method,
        PkceMethod::Plain
    );
    assert_eq!(
        pkce_for_kind(ProviderKind::InHouse).method,
        PkceMethod::S256
    );
}

#[test]
fn status_snake_round_trips_every_variant() {
    for s in [
        ListStatus::Planning,
        ListStatus::Watching,
        ListStatus::Completed,
        ListStatus::Paused,
        ListStatus::Dropped,
        ListStatus::Rewatching,
    ] {
        assert_eq!(status_from_snake(status_to_snake(s)), Some(s));
    }
}

#[test]
fn status_from_snake_returns_none_for_unknown() {
    assert_eq!(status_from_snake(""), None);
    assert_eq!(status_from_snake("Planning"), None);
    assert_eq!(status_from_snake("plan_to_watch"), None);
}

#[test]
fn tokens_from_bearer_drops_expiry_and_refresh() {
    let t = tokens_from_bearer("xyz");
    assert_eq!(t.access_token, "xyz");
    assert!(t.refresh_token.is_none());
    assert_eq!(t.expires_at_epoch_s, 0);
}

#[test]
fn watch_later_bridge_max_ids_is_a_sane_ceiling() {
    // Codex P1 #3373789621: the route gates on this constant.
    // 500 covers the largest plausible Plan-to-Watch (a heavy
    // listmaker with curated picks tops out around 200-300) and
    // bounds per-request fan-out cost. Pinned here so a future
    // bump is intentional, not a typo.
    assert_eq!(WATCH_LATER_BRIDGE_MAX_IDS, 500);
}

#[test]
fn upsert_cached_entry_writes_through_to_the_cache() {
    // Codex P2 #3412673593: the write-back path upserts the just-synced
    // entry so the Watch Later rail sees the new status without a full
    // resync. Exercise the commands wrapper end-to-end against an
    // in-memory pool.
    use crate::account::provider::{ListEntry, ProviderKind, ProviderMediaId};
    use crate::account::status::ListStatus;
    let state = state_with_kitsu("http://127.0.0.1:0");
    let entry = ListEntry {
        provider: ProviderKind::AniList,
        media_id: ProviderMediaId(5),
        mal_id: Some(5),
        status: ListStatus::Watching,
        progress_episodes: 2,
        score_0_to_100: None,
        updated_at_epoch_s: 0,
        title: "X".into(),
    };
    upsert_cached_entry(&state, ProviderKind::AniList, "u", &entry).unwrap();
    let got = cached_list(&state, ProviderKind::AniList, "u").unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].status, ListStatus::Watching);
}
