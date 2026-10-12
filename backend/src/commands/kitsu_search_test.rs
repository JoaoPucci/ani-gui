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
