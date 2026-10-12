//! A search page knows where the next one starts.

use super::*;
use crate::meta::kitsu::{parse_search_page, KitsuClient};
use proptest::prelude::*;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SEARCH_FIXTURE: &[u8] = include_bytes!("../../../tests/fixtures/kitsu/search_one_piece.json");

fn anime(id: &str, subtype: &str) -> serde_json::Value {
    json!({
        "id": id,
        "type": "anime",
        "attributes": { "canonicalTitle": format!("Show {id}"), "subtype": subtype }
    })
}

/// A body with `rows` rows, `links.next` when `next` is set, and
/// `meta.count` when `count` is.
fn body(rows: &[serde_json::Value], next: bool, count: Option<u64>) -> Vec<u8> {
    let mut v = json!({ "data": rows, "links": { "first": "https://kitsu.io/api/edge/anime" } });
    if next {
        v["links"]["next"] = json!("https://kitsu.io/api/edge/anime?page%5Boffset%5D=20");
    }
    if let Some(c) = count {
        v["meta"] = json!({ "count": c });
    }
    serde_json::to_vec(&v).expect("json")
}

#[test]
fn a_page_with_a_next_link_points_one_page_on() {
    // The recorded fixture is a five-row page of 156 with `links.next`.
    let page = parse_search_page(SEARCH_FIXTURE, 0, 5).expect("parses");
    assert_eq!(page.hits.len(), 5);
    assert_eq!(page.next_offset, Some(5));
}

#[test]
fn the_last_page_has_no_next_offset() {
    let rows = [anime("1", "TV"), anime("2", "TV")];
    let page = parse_search_page(&body(&rows, false, Some(42)), 40, 20).expect("parses");
    assert_eq!(page.hits.len(), 2);
    assert_eq!(page.next_offset, None);
}

#[test]
fn the_count_says_more_when_the_links_are_missing() {
    let rows: Vec<_> = (0..20).map(|i| anime(&i.to_string(), "TV")).collect();
    let mut v: serde_json::Value = serde_json::from_slice(&body(&rows, false, Some(68))).unwrap();
    v.as_object_mut().unwrap().remove("links");
    let page = parse_search_page(&serde_json::to_vec(&v).unwrap(), 20, 20).expect("parses");
    assert_eq!(page.next_offset, Some(40));
}

#[test]
fn an_empty_page_ends_the_search_whatever_its_links_say() {
    // A page past the end with a `next` link anyway must not send a
    // caller round again: nothing came back, so nothing lies beyond.
    let page = parse_search_page(&body(&[], true, Some(500)), 480, 20).expect("parses");
    assert!(page.hits.is_empty());
    assert_eq!(page.next_offset, None);
}

#[test]
fn dropped_music_does_not_end_the_search() {
    // The next offset counts Kitsu's rows, not the hits left after the
    // music filter: a page of nothing but music videos still has a
    // page after it.
    let rows = [anime("1", "music"), anime("2", "music")];
    let page = parse_search_page(&body(&rows, true, Some(80)), 20, 20).expect("parses");
    assert!(page.hits.is_empty());
    assert_eq!(page.next_offset, Some(40));
}

proptest! {
    #[test]
    fn the_next_offset_is_one_page_on_or_none(
        offset in 0u32..100_000,
        limit in 1u8..=20,
        returned in 0usize..=20,
        has_next_link: bool,
        count in proptest::option::of(0u64..200_000),
    ) {
        let next = next_offset(offset, limit, returned, has_next_link, count);
        if let Some(n) = next {
            prop_assert_eq!(n, offset + u32::from(limit));
        }
        if returned == 0 {
            prop_assert_eq!(next, None);
        }
        if returned > 0 && has_next_link {
            prop_assert_eq!(next, Some(offset + u32::from(limit)));
        }
        if returned > 0 && !has_next_link {
            let more = count.is_some_and(|c| c > u64::from(offset) + u64::from(limit));
            prop_assert_eq!(next.is_some(), more);
        }
    }
}

#[tokio::test]
async fn the_client_asks_kitsu_for_the_offset_it_was_given() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .and(query_param("filter[text]", "dragon ball"))
        .and(query_param("page[limit]", "20"))
        .and(query_param("page[offset]", "60"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(body(&[anime("68", "TV")], true, Some(120))),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = KitsuClient::with_base(reqwest::Client::new(), mock.uri());
    let page = client.search_page("dragon ball", 20, 60).await.expect("ok");
    assert_eq!(page.hits[0].id, "68");
    assert_eq!(page.next_offset, Some(80));
}

#[tokio::test]
async fn a_failed_page_is_an_upstream_error() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), mock.uri());
    let err = client.search_page("x", 20, 20).await.expect_err("503");
    assert!(matches!(
        err,
        crate::error::AniError::Upstream { status: 503 }
    ));
}
