//! A search page knows where the next one starts.

use super::*;
use crate::meta::kitsu::{parse_search_page, KitsuClient};
use proptest::prelude::*;
use serde_json::json;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
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

/// Kitsu's text search reads `page[offset]` a page late: offsets 0 and
/// 20 both answer the first page, 40 the second, so the last page of a
/// query is never asked for. Measured on "dragon ball" (58 hits): offset
/// 60 answered the third page, which offset 40 should have. Its
/// `page[number]` answers the page asked for, so the client turns the
/// offset into the page that starts there.
#[tokio::test]
async fn the_client_asks_kitsu_for_the_page_that_starts_at_the_offset() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .and(query_param("filter[text]", "dragon ball"))
        .and(query_param("page[size]", "20"))
        .and(query_param("page[number]", "4"))
        .and(query_param_is_missing("page[offset]"))
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

/// Kitsu sorts a text search itself when asked: `-startDate` for
/// newest first, `-averageRating` for best rated first, and pages a
/// sorted search as consistently as a ranked one. Its own ranking is
/// asked for by sending no sort at all.
#[tokio::test]
async fn the_client_asks_kitsu_to_sort_the_search_in_the_order_given() {
    use crate::meta::kitsu_page::SearchOrder;
    let mock = MockServer::start().await;
    for (sort, id) in [("-startDate", "year"), ("-averageRating", "rating")] {
        Mock::given(method("GET"))
            .and(path("/anime"))
            .and(query_param("sort", sort))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(body(&[anime(id, "TV")], false, Some(1))),
            )
            .expect(1)
            .mount(&mock)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/anime"))
        .and(query_param_is_missing("sort"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(body(&[anime("relevance", "TV")], false, Some(1))),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = KitsuClient::with_base(reqwest::Client::new(), mock.uri());
    for (order, id) in [
        (SearchOrder::Year, "year"),
        (SearchOrder::Rating, "rating"),
        (SearchOrder::Relevance, "relevance"),
    ] {
        let page = client
            .search_page_ordered("dragon ball", 20, 0, order)
            .await
            .expect("ok");
        assert_eq!(page.hits[0].id, id, "{order:?}");
    }
}

/// A ranked search adds no parameter in place of the sort it leaves
/// out: every query parameter it sends has a name.
#[tokio::test]
async fn a_ranked_search_sends_no_nameless_parameter() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(body(&[anime("1", "TV")], false, Some(1))),
        )
        .mount(&mock)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), mock.uri());
    client.search_page("dragon ball", 20, 0).await.expect("ok");
    let requests = mock.received_requests().await.expect("recorded");
    let query = requests[0].url.query().unwrap_or_default().to_string();
    for pair in query.split('&') {
        assert!(!pair.starts_with('='), "nameless parameter in {query}");
    }
}

/// Kitsu's `filter[text]` reads a word that starts with `-` as one to
/// exclude: the provider's "Re:ZERO -Starting Life in Another World-
/// Season 4" answers Zoids Genesis alone, and the same title without
/// that `-` answers Re:ZERO Season 4. The client sends the word.
#[test]
fn a_word_led_by_a_dash_is_sent_as_the_word() {
    assert_eq!(
        search_text("Re:ZERO -Starting Life in Another World- Season 4"),
        "Re:ZERO Starting Life in Another World- Season 4"
    );
    assert_eq!(search_text("zero --kara"), "zero kara");
    assert_eq!(search_text("-naruto"), "naruto");
}

/// A dash inside or after a word, or a dash other than `-`, is text
/// to Kitsu, and reaches it as typed.
#[test]
fn a_dash_inside_or_after_a_word_is_left_alone() {
    for q in [
        "Re-Zero",
        "naruto- shippuden",
        "zero \u{2014}kara",
        "zero \u{2212}kara",
        "[Oshi no Ko]",
    ] {
        assert_eq!(search_text(q), q, "{q}");
    }
}

/// A dash standing alone leads an empty word: Kitsu answers a lone `-`
/// as it answers nothing at all, so it is not sent.
#[test]
fn a_lone_dash_is_not_sent() {
    assert_eq!(
        search_text("Bleach: Thousand-Year Blood War - The Calamity"),
        "Bleach: Thousand-Year Blood War The Calamity"
    );
    assert_eq!(search_text("-"), "");
}

const REZERO_DASHED: &[u8] =
    include_bytes!("../../../tests/fixtures/kitsu/search_rezero_season_4_dashed.json");
const REZERO: &[u8] = include_bytes!("../../../tests/fixtures/kitsu/search_rezero_season_4.json");

/// Recorded answers: Kitsu gives the provider's Re:ZERO Season 4 title
/// Zoids Genesis as typed, and Re:ZERO Season 4 once the dash that
/// led "Starting" is gone.
#[tokio::test]
async fn the_client_finds_re_zero_season_4_by_its_provider_title() {
    let mock = MockServer::start().await;
    for (text, recorded) in [
        (
            "Re:ZERO -Starting Life in Another World- Season 4",
            REZERO_DASHED,
        ),
        ("Re:ZERO Starting Life in Another World- Season 4", REZERO),
    ] {
        Mock::given(method("GET"))
            .and(path("/anime"))
            .and(query_param("filter[text]", text))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(recorded.to_vec()),
            )
            .mount(&mock)
            .await;
    }
    let client = KitsuClient::with_base(reqwest::Client::new(), mock.uri());
    let hits = client
        .search("Re:ZERO -Starting Life in Another World- Season 4", 20)
        .await
        .expect("ok");
    let ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
    assert_eq!(ids, ["49746"]);
}

proptest! {
    /// No word Kitsu is sent starts with a dash, every word of the
    /// query reaches it less its leading dashes, and sending the text
    /// again sends the same text.
    #[test]
    fn no_word_sent_is_an_exclusion(
        words in prop::collection::vec("-{0,3}[A-Za-z0-9:!'\\-]{0,6}", 0..8),
        gaps in prop::collection::vec("[ \\t]{1,3}", 8),
    ) {
        let query: String = words
            .iter()
            .zip(&gaps)
            .map(|(w, g)| format!("{w}{g}"))
            .collect();
        let sent = search_text(&query);
        for word in sent.split_whitespace() {
            prop_assert!(!word.starts_with('-'), "{:?} sent as {:?}", query, sent);
        }
        let kept: Vec<&str> = query
            .split_whitespace()
            .map(|w| w.trim_start_matches('-'))
            .filter(|w| !w.is_empty())
            .collect();
        prop_assert_eq!(sent.split_whitespace().collect::<Vec<_>>(), kept);
        prop_assert_eq!(search_text(&sent), sent.clone());
    }

    /// A query no word of which starts with a dash is sent as typed,
    /// up to the space between its words.
    #[test]
    fn a_query_without_an_exclusion_is_sent_as_typed(
        words in prop::collection::vec("[A-Za-z0-9:!'][A-Za-z0-9:!'\\-]{0,6}", 1..8),
    ) {
        let query = words.join(" ");
        prop_assert_eq!(search_text(&query), query);
    }
}
