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
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
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

/// Mounts the page that starts at `offset`, which Kitsu is asked for
/// by number: offset 0 is page 1, offset 20 page 2.
async fn mount_page(mock: &MockServer, offset: &str, ids: &[&str], next: bool, times: u64) {
    let number = offset.parse::<u32>().expect("offset") / 20 + 1;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .and(query_param("page[number]", number.to_string()))
        .and(query_param("page[size]", "20"))
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

#[test]
fn a_page_request_reads_its_query_and_offset() {
    let (q, o) = read_page_request(br#"{"query":"dragon ball","offset":40}"#).expect("ok");
    assert_eq!((q.as_str(), o), ("dragon ball", 40));
}

#[test]
fn a_body_that_is_not_a_page_request_is_an_invalid_body() {
    for body in [
        &b"not json"[..],
        br#"{"offset":20}"#,
        br#"{"query":7,"offset":20}"#,
        b"[]",
    ] {
        let err = read_page_request(body).expect_err("refused");
        assert!(
            matches!(err, crate::error::AniError::InvalidRequestBody),
            "{}: {err:?}",
            String::from_utf8_lossy(body)
        );
    }
}

#[test]
fn an_offset_that_is_no_page_offset_is_an_invalid_offset() {
    for body in [
        &br#"{"query":"x"}"#[..],
        br#"{"query":"x","offset":null}"#,
        br#"{"query":"x","offset":-20}"#,
        br#"{"query":"x","offset":20.5}"#,
        br#"{"query":"x","offset":"20"}"#,
        br#"{"query":"x","offset":4294967296}"#,
    ] {
        let err = read_page_request(body).expect_err("refused");
        assert!(
            matches!(err, crate::error::AniError::InvalidSearchOffset),
            "{}: {err:?}",
            String::from_utf8_lossy(body)
        );
    }
}

proptest::proptest! {
    /// Only the three orders the route offers are read; any other
    /// `sort` string is refused rather than read as relevance.
    #[test]
    fn only_an_offered_order_is_read(sort in proptest::prop_oneof![
        proptest::strategy::Just("relevance".to_owned()),
        proptest::strategy::Just("year".to_owned()),
        proptest::strategy::Just("rating".to_owned()),
        proptest::strategy::Just("Year".to_owned()),
        proptest::strategy::Just("years".to_owned()),
        proptest::strategy::Just("title".to_owned()),
        "[a-zA-Z_]{0,12}",
    ]) {
        use crate::meta::kitsu_page::SearchOrder;
        let body = format!(r#"{{"query":"q","offset":0,"sort":"{sort}"}}"#);
        let want = match sort.as_str() {
            "relevance" => Some(SearchOrder::Relevance),
            "year" => Some(SearchOrder::Year),
            "rating" => Some(SearchOrder::Rating),
            _ => None,
        };
        match want {
            Some(order) => proptest::prop_assert_eq!(read_page_order(body.as_bytes()).ok(), Some(order)),
            None => proptest::prop_assert!(matches!(
                read_page_order(body.as_bytes()),
                Err(crate::error::AniError::InvalidRequestBody)
            )),
        }
    }

    #[test]
    fn any_integer_offset_reads_as_itself_or_is_refused(offset in proptest::num::i64::ANY) {
        let body = format!(r#"{{"query":"q","offset":{offset}}}"#);
        match u32::try_from(offset) {
            Ok(o) => proptest::prop_assert_eq!(read_page_request(body.as_bytes()).ok(), Some(("q".to_owned(), o))),
            Err(_) => proptest::prop_assert!(matches!(
                read_page_request(body.as_bytes()),
                Err(crate::error::AniError::InvalidSearchOffset)
            )),
        }
    }
}

/// A page cached before the client asked Kitsu by page number holds
/// what Kitsu's late offsets answered, often the page before. It is not
/// served: the page is fetched again.
#[tokio::test]
async fn a_page_cached_from_an_offset_request_is_fetched_again() {
    let mock = MockServer::start().await;
    mount_page(&mock, "20", &["21"], true, 1).await;

    let state = state_with_kitsu_at(&mock.uri());
    let stale = serde_json::to_string(&crate::meta::kitsu_page::KitsuSearchPage {
        hits: Vec::new(),
        next_offset: Some(40),
    })
    .expect("json");
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "kitsu:v3:search:20:dragon ball",
        &stale,
        3600,
    )
    .expect("seed");
    let page = kitsu_search_page(&state, "dragon ball", 20)
        .await
        .expect("ok");
    assert_eq!(page.hits[0].id, "21");
}

/// Each order is its own search: a page of one is never served as the
/// same page of another.
#[tokio::test]
async fn each_order_is_fetched_and_cached_on_its_own() {
    use crate::meta::kitsu_page::SearchOrder;
    let mock = MockServer::start().await;
    for (sort, id) in [("-startDate", "new"), ("-averageRating", "best")] {
        Mock::given(method("GET"))
            .and(path("/anime"))
            .and(query_param("sort", sort))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(page_body(&[id], false)),
            )
            .expect(1)
            .mount(&mock)
            .await;
    }
    mount_page(&mock, "0", &["ranked"], false, 1).await;

    let state = state_with_kitsu_at(&mock.uri());
    for _ in 0..2 {
        for (order, id) in [
            (SearchOrder::Relevance, "ranked"),
            (SearchOrder::Year, "new"),
            (SearchOrder::Rating, "best"),
        ] {
            let page = kitsu_search_page_ordered(&state, "dragon ball", 0, order)
                .await
                .expect("ok");
            assert_eq!(page.hits[0].id, id, "{order:?}");
        }
    }
}

#[test]
fn a_page_request_reads_its_order() {
    use crate::meta::kitsu_page::SearchOrder;
    for (body, order) in [
        (&br#"{"query":"q","offset":0}"#[..], SearchOrder::Relevance),
        (
            br#"{"query":"q","offset":0,"sort":"relevance"}"#,
            SearchOrder::Relevance,
        ),
        (
            br#"{"query":"q","offset":0,"sort":"year"}"#,
            SearchOrder::Year,
        ),
        (
            br#"{"query":"q","offset":0,"sort":"rating"}"#,
            SearchOrder::Rating,
        ),
    ] {
        assert_eq!(read_page_order(body).expect("ok"), order);
    }
}

/// An order the route does not offer is a malformed request, not a
/// silent fallback to relevance: the renderer only sends the three.
#[test]
fn an_order_the_route_does_not_offer_is_an_invalid_body() {
    for body in [
        &br#"{"query":"q","offset":0,"sort":"title"}"#[..],
        br#"{"query":"q","offset":0,"sort":7}"#,
    ] {
        let err = read_page_order(body).expect_err("refused");
        assert!(matches!(err, AniError::InvalidRequestBody), "{err:?}");
    }
}
