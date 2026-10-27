//! Tests for `crate::commands::external_ids`. Extracted via `#[path]`
//! so the wiremock fixtures stay off the command file's CRAP budget.

use std::path::PathBuf;
use std::sync::Arc;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::external_ids_for;
use crate::app::AppState;
use crate::cache::meta_cache_row;
use crate::error::AniError;
use crate::meta::kitsu::ExternalIds;

/// `AppState` whose Kitsu client points at `kitsu_uri`; everything
/// else is throwaway.
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

/// Kitsu `/anime/:id?include=mappings` body carrying the given
/// `(externalSite, externalId)` rows.
fn mappings_body(kitsu_id: &str, sites: &[(&str, u32)]) -> String {
    let included: Vec<serde_json::Value> = sites
        .iter()
        .enumerate()
        .map(|(i, (site, id))| {
            serde_json::json!({
                "id": format!("{i}"),
                "type": "mappings",
                "attributes": { "externalSite": site, "externalId": id.to_string() }
            })
        })
        .collect();
    serde_json::json!({
        "data": { "id": kitsu_id, "type": "anime", "attributes": { "canonicalTitle": "x" } },
        "included": included,
    })
    .to_string()
}

/// A Kitsu answering `/anime/:id` with `body`, `times` times at most.
async fn kitsu_answering(kitsu_id: &str, template: ResponseTemplate, times: u64) -> MockServer {
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/anime/{kitsu_id}")))
        .respond_with(template)
        .expect(times)
        .mount(&kitsu)
        .await;
    kitsu
}

const DAY: u64 = 24 * 60 * 60;

#[tokio::test]
async fn a_second_read_for_the_same_show_asks_kitsu_nothing() {
    let body = mappings_body("12", &[("myanimelist/anime", 21), ("anilist/anime", 21)]);
    let kitsu = kitsu_answering("12", ResponseTemplate::new(200).set_body_string(body), 1).await;
    let state = state_with_kitsu(&kitsu.uri());

    let first = external_ids_for(&state, "12").await.expect("first read");
    let second = external_ids_for(&state, "12").await.expect("second read");

    let want = ExternalIds {
        mal: Some(21),
        anilist: Some(21),
    };
    assert_eq!(first, want);
    assert_eq!(second, want, "the cached answer carries both ids");
    // The MockServer verifies `.expect(1)` on drop.
}

#[tokio::test]
async fn a_kitsu_failure_is_not_remembered() {
    let kitsu = kitsu_answering("12", ResponseTemplate::new(503), 2).await;
    let state = state_with_kitsu(&kitsu.uri());

    for _ in 0..2 {
        let err = external_ids_for(&state, "12").await.unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 503 }), "{err:?}");
    }
    // `.expect(2)`: the second read went back to Kitsu.
}

#[tokio::test]
async fn a_complete_mapping_is_kept_for_thirty_days() {
    let body = mappings_body("12", &[("myanimelist/anime", 21), ("anilist/anime", 21)]);
    let kitsu = kitsu_answering("12", ResponseTemplate::new(200).set_body_string(body), 1).await;
    let state = state_with_kitsu(&kitsu.uri());

    external_ids_for(&state, "12").await.expect("read");

    let row = meta_cache_row(&state.cache_pool, "kitsu:external-ids:v2:k12")
        .expect("cache read")
        .expect("row written");
    assert_eq!(row.ttl_seconds, 30 * DAY);
}

#[tokio::test]
async fn a_mapping_missing_a_tracker_is_kept_for_a_day() {
    // A fresh seasonal show: Kitsu carries the AniList mapping and
    // adds MAL's within days. What is missing may appear; what is
    // present does not move.
    let body = mappings_body("50497", &[("anilist/anime", 185660)]);
    let kitsu = kitsu_answering("50497", ResponseTemplate::new(200).set_body_string(body), 1).await;
    let state = state_with_kitsu(&kitsu.uri());

    let ids = external_ids_for(&state, "50497").await.expect("read");
    assert_eq!(
        ids,
        ExternalIds {
            mal: None,
            anilist: Some(185660),
        }
    );

    let row = meta_cache_row(&state.cache_pool, "kitsu:external-ids:v2:k50497")
        .expect("cache read")
        .expect("row written");
    assert_eq!(row.ttl_seconds, DAY);
}

#[tokio::test]
async fn a_show_with_no_mapping_at_all_is_kept_for_a_day_too() {
    let body = mappings_body("999", &[]);
    let kitsu = kitsu_answering("999", ResponseTemplate::new(200).set_body_string(body), 1).await;
    let state = state_with_kitsu(&kitsu.uri());

    let first = external_ids_for(&state, "999").await.expect("read");
    let second = external_ids_for(&state, "999").await.expect("read");
    assert_eq!(first, ExternalIds::default());
    assert_eq!(second, ExternalIds::default());

    let row = meta_cache_row(&state.cache_pool, "kitsu:external-ids:v2:k999")
        .expect("cache read")
        .expect("row written");
    assert_eq!(row.ttl_seconds, DAY);
}

#[tokio::test]
async fn a_row_this_build_cannot_read_is_fetched_again_and_overwritten() {
    // The key is versioned for a change of shape; a row that still does
    // not parse — a hand-edited database, a half-written body — is not
    // an answer, and must not strand the show on it.
    let body = mappings_body("12", &[("myanimelist/anime", 21), ("anilist/anime", 21)]);
    let kitsu = kitsu_answering("12", ResponseTemplate::new(200).set_body_string(body), 1).await;
    let state = state_with_kitsu(&kitsu.uri());
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "kitsu:external-ids:v2:k12",
        "not the shape",
        DAY,
    )
    .expect("seed");

    let ids = external_ids_for(&state, "12").await.expect("read");
    assert_eq!(
        ids,
        ExternalIds {
            mal: Some(21),
            anilist: Some(21),
        }
    );
    let row = meta_cache_row(&state.cache_pool, "kitsu:external-ids:v2:k12")
        .expect("cache read")
        .expect("row");
    assert_eq!(
        serde_json::from_str::<ExternalIds>(&row.body).expect("overwritten with the shape"),
        ids
    );
}

#[tokio::test]
async fn two_concurrent_cold_reads_of_one_show_cost_one_kitsu_request() {
    // The detail page reads both trackers' entries in one go, and its
    // airing and banner lookups start from the same mappings; on a
    // cold show all of them miss the cache in the same instant. The
    // second read waits for the first and takes its row.
    let body = mappings_body("12", &[("myanimelist/anime", 21), ("anilist/anime", 21)]);
    let kitsu = kitsu_answering(
        "12",
        ResponseTemplate::new(200)
            .set_body_string(body)
            .set_delay(std::time::Duration::from_millis(300)),
        1,
    )
    .await;
    let state = state_with_kitsu(&kitsu.uri());

    let (first, second) = tokio::join!(
        external_ids_for(&state, "12"),
        external_ids_for(&state, "12")
    );

    let want = ExternalIds {
        mal: Some(21),
        anilist: Some(21),
    };
    assert_eq!(first.expect("first read"), want);
    assert_eq!(second.expect("second read"), want);
    // `.expect(1)`: one Kitsu request served both.
}

/// A v1 row with no ids may be what an earlier build kept from a reply
/// that was not Kitsu's anime document at all, read then as a show
/// with no mappings. Nothing in the row tells the two apart, so v1 rows
/// are not read: the key moved past them, and the show is asked again.
#[tokio::test]
async fn an_empty_row_an_earlier_build_kept_is_not_read() {
    let body = mappings_body("12", &[("myanimelist/anime", 21)]);
    let kitsu = kitsu_answering("12", ResponseTemplate::new(200).set_body_string(body), 1).await;
    let state = state_with_kitsu(&kitsu.uri());
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "kitsu:external-ids:v1:k12",
        &serde_json::to_string(&ExternalIds::default()).expect("json"),
        DAY,
    )
    .expect("seed");
    let ids = external_ids_for(&state, "12").await.expect("read");
    assert_eq!(ids.mal, Some(21), "asked again rather than read from v1");
}
