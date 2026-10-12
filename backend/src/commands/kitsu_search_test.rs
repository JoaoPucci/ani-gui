//! Tests for the cached Kitsu text search.

use super::*;
use std::path::PathBuf;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SEARCH_FIXTURE: &[u8] = include_bytes!("../../../tests/fixtures/kitsu/search_one_piece.json");

fn state_with_kitsu_at(uri: &str) -> AppState {
    AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
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

#[test]
fn normalize_query_collapses_whitespace_and_lowercases() {
    assert_eq!(normalize_query("One Piece"), "one piece");
    assert_eq!(normalize_query("  ONE   PIECE  "), "one piece");
    assert_eq!(normalize_query("\tone\npiece"), "one piece");
}

#[tokio::test]
async fn kitsu_search_caches_after_first_call() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .and(query_param("filter[text]", "one piece"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(SEARCH_FIXTURE.to_vec()),
        )
        .expect(1) // <-- must only hit upstream once
        .mount(&mock)
        .await;

    let state = state_with_kitsu_at(&mock.uri());
    let first = kitsu_search(&state, "one piece").await.expect("ok");
    let second = kitsu_search(&state, "one piece").await.expect("ok");
    assert_eq!(first.len(), 5);
    assert_eq!(first, second, "cache hit returns identical body");
}

#[tokio::test]
async fn kitsu_search_normalizes_query_for_cache_key() {
    // Two queries that normalize to the same key should share the
    // same cached entry — upstream sees one request total.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(SEARCH_FIXTURE.to_vec()),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let state = state_with_kitsu_at(&mock.uri());
    let _ = kitsu_search(&state, "  One   Piece  ").await.unwrap();
    let _ = kitsu_search(&state, "ONE PIECE").await.unwrap();
    // mock.expect(1) does the assertion when the harness drops.
}

#[tokio::test]
async fn kitsu_search_propagates_upstream_errors_and_does_not_cache() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .respond_with(ResponseTemplate::new(503))
        .expect(2) // <-- both calls hit upstream because errors are not cached
        .mount(&mock)
        .await;

    let state = state_with_kitsu_at(&mock.uri());
    assert!(kitsu_search(&state, "anything").await.is_err());
    assert!(kitsu_search(&state, "anything").await.is_err());
}

/// A Kitsu page body of `ids`, with `links.next` when `next` is set.
fn page_body(ids: &[&str], next: bool) -> Vec<u8> {
    let rows: Vec<_> = ids
        .iter()
        .map(|id| {
            serde_json::json!({
                "id": id,
                "type": "anime",
                "attributes": { "canonicalTitle": format!("Show {id}"), "subtype": "TV" }
            })
        })
        .collect();
    let mut v = serde_json::json!({ "data": rows, "links": {}, "meta": { "count": 2000 } });
    if next {
        v["links"]["next"] =
            serde_json::json!("https://kitsu.io/api/edge/anime?page%5Boffset%5D=1");
    } else {
        v["meta"]["count"] = serde_json::json!(0);
    }
    serde_json::to_vec(&v).expect("json")
}

async fn mount_page(mock: &MockServer, offset: &str, ids: &[&str], next: bool, times: u64) {
    Mock::given(method("GET"))
        .and(path("/anime"))
        .and(query_param("page[offset]", offset))
        .and(query_param("page[limit]", "20"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(page_body(ids, next)),
        )
        .expect(times)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn each_page_is_fetched_once_and_cached_under_its_own_offset() {
    let mock = MockServer::start().await;
    mount_page(&mock, "0", &["1"], true, 1).await;
    mount_page(&mock, "20", &["21"], true, 1).await;

    let state = state_with_kitsu_at(&mock.uri());
    let first = kitsu_search_page(&state, "dragon ball", 0)
        .await
        .expect("ok");
    let second = kitsu_search_page(&state, "Dragon  Ball", 20)
        .await
        .expect("ok");
    assert_eq!(first.hits[0].id, "1");
    assert_eq!(first.next_offset, Some(20));
    assert_eq!(second.hits[0].id, "21");
    assert_eq!(second.next_offset, Some(40));
    // Served from the cache the second time round: mock.expect(1).
    assert_eq!(
        kitsu_search_page(&state, "dragon ball", 20)
            .await
            .expect("ok"),
        second
    );
}

#[tokio::test]
async fn the_first_page_and_the_plain_search_share_one_fetch() {
    let mock = MockServer::start().await;
    mount_page(&mock, "0", &["1", "2"], true, 1).await;

    let state = state_with_kitsu_at(&mock.uri());
    let page = kitsu_search_page(&state, "dragon ball", 0)
        .await
        .expect("ok");
    let hits = kitsu_search(&state, "dragon ball").await.expect("ok");
    assert_eq!(page.hits, hits);
}

#[tokio::test]
async fn an_offset_off_a_page_boundary_or_past_the_deepest_page_is_refused_unfetched() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(page_body(&[], false)))
        .expect(0)
        .mount(&mock)
        .await;

    let state = state_with_kitsu_at(&mock.uri());
    for offset in [7, 19, 21, MAX_SEARCH_OFFSET + 20, u32::MAX] {
        let err = kitsu_search_page(&state, "x", offset)
            .await
            .expect_err("refused");
        assert!(
            matches!(err, crate::error::AniError::InvalidSearchOffset),
            "{offset}: {err:?}"
        );
    }
}

#[tokio::test]
async fn the_deepest_page_has_no_next_offset_even_when_kitsu_has_more() {
    // Never advertise an offset the route would refuse.
    let mock = MockServer::start().await;
    mount_page(&mock, &MAX_SEARCH_OFFSET.to_string(), &["1"], true, 1).await;

    let state = state_with_kitsu_at(&mock.uri());
    let page = kitsu_search_page(&state, "x", MAX_SEARCH_OFFSET)
        .await
        .expect("ok");
    assert_eq!(page.hits.len(), 1);
    assert_eq!(page.next_offset, None);
}

#[tokio::test]
async fn a_failed_page_is_not_cached() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .respond_with(ResponseTemplate::new(503))
        .expect(2)
        .mount(&mock)
        .await;

    let state = state_with_kitsu_at(&mock.uri());
    assert!(kitsu_search_page(&state, "x", 20).await.is_err());
    assert!(kitsu_search_page(&state, "x", 20).await.is_err());
}
