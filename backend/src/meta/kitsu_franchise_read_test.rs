//! The franchise relations reader against Kitsu's recorded answer and
//! hand-written pages.

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/kitsu")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The 2000 JoJo OVA lists the 1993 OVA as its sequel, the 2012 series
/// as its prequel, Stardust Crusaders as another version and the manga
/// as its source, each anime with what a strip shows of it.
#[test]
fn the_jojo_ova_s_listing_reads_as_kitsu_recorded_it() {
    let page = parse_franchise_page(&fixture("franchise_rel_7730.json")).expect("parsed");
    assert_eq!(page.read, 4);
    assert_eq!(page.count, Some(4));
    let read: Vec<(&str, &str, &str)> = page
        .relations
        .iter()
        .map(|r| (r.role.as_str(), r.kind.as_str(), r.id.as_str()))
        .collect();
    assert_eq!(
        read,
        vec![
            ("sequel", "anime", "610"),
            ("alternative_version", "anime", "8063"),
            ("prequel", "anime", "7158"),
            ("adaptation", "manga", "1899"),
        ]
    );
    let ova = page.relations[0].entry.as_ref().expect("an anime entry");
    assert_eq!(ova.title, "JoJo no Kimyou na Bouken");
    assert_eq!(ova.subtype.as_deref(), Some("OVA"));
    assert_eq!(ova.start.as_deref(), Some("1993-11-19"));
    assert_eq!(ova.episodes, Some(6));
    assert_eq!(ova.status.as_deref(), Some("finished"));
    assert!(ova.poster.as_deref().is_some_and(|p| p.contains("medium")));
    assert!(page.relations[3].entry.is_none(), "a manga has no entry");
    assert_eq!(page.relations[3].subtype.as_deref(), Some("manga"));
}

fn anime_node(id: &str) -> serde_json::Value {
    json!({ "type": "anime", "id": id, "attributes": {
        "canonicalTitle": format!("T{id}"), "subtype": "TV", "status": "finished",
        "episodeCount": 12, "startDate": "2020-01-01",
        "posterImage": { "small": "s.jpg", "original": "o.jpg" },
        "coverImage": null
    }})
}

fn edge(role: &str, id: &str) -> serde_json::Value {
    json!({ "type": "mediaRelationships", "attributes": { "role": role },
        "relationships": { "destination": { "data": { "type": "anime", "id": id } } } })
}

/// The poster falls back to the small size when there is no medium,
/// and a null cover is none.
#[test]
fn artwork_falls_back_by_size() {
    let body = json!({ "data": [edge("sequel", "1")], "included": [anime_node("1")], "meta": { "count": 1 } });
    let page = parse_franchise_page(&serde_json::to_vec(&body).expect("ser")).expect("parsed");
    let e = page.relations[0].entry.as_ref().expect("entry");
    assert_eq!(e.poster.as_deref(), Some("s.jpg"));
    assert_eq!(e.cover, None);
}

/// A destination the page does not include is no answer.
#[test]
fn a_destination_not_included_is_no_answer() {
    let body = json!({ "data": [edge("sequel", "1")], "included": [], "meta": { "count": 1 } });
    assert!(parse_franchise_page(&serde_json::to_vec(&body).expect("ser")).is_err());
}

/// An anime that leaves out its title is no answer.
#[test]
fn an_anime_without_its_title_is_no_answer() {
    let mut node = anime_node("1");
    node["attributes"]
        .as_object_mut()
        .expect("attrs")
        .remove("canonicalTitle");
    let body = json!({ "data": [edge("sequel", "1")], "included": [node], "meta": { "count": 1 } });
    assert!(parse_franchise_page(&serde_json::to_vec(&body).expect("ser")).is_err());
}

/// Paging stops at the count, at an empty page, or at the cap.
#[test]
fn paging_follows_the_count_up_to_the_cap() {
    assert!(more_pages(1, 20, 20, Some(25)));
    assert!(!more_pages(2, 25, 5, Some(25)));
    assert!(!more_pages(1, 20, 0, Some(25)));
    assert!(!more_pages(FRANCHISE_MAX_PAGES, 100, 20, Some(150)));
    assert!(!more_pages(1, 3, 3, None));
    assert!(listing_complete(25, Some(25)));
    assert!(!listing_complete(100, Some(150)));
    assert!(!listing_complete(3, None));
}

fn page_body(ids: std::ops::Range<u32>, count: usize) -> serde_json::Value {
    let edges: Vec<_> = ids
        .clone()
        .map(|i| edge("side_story", &i.to_string()))
        .collect();
    let nodes: Vec<_> = ids.map(|i| anime_node(&i.to_string())).collect();
    json!({ "data": edges, "included": nodes, "meta": { "count": count } })
}

/// A listing Kitsu counts past one page is read page by page, each
/// asked at the offset the pages before it reached.
#[tokio::test]
async fn a_long_listing_is_read_page_by_page() {
    let server = MockServer::start().await;
    for (offset, ids) in [("0", 1..21), ("20", 21..26)] {
        Mock::given(method("GET"))
            .and(path("/anime/9/media-relationships"))
            .and(query_param("page[offset]", offset))
            .and(query_param("fields[anime]", FRANCHISE_ANIME_FIELDS))
            .respond_with(ResponseTemplate::new(200).set_body_json(page_body(ids, 25)))
            .expect(1)
            .mount(&server)
            .await;
    }
    let client = KitsuClient::with_base(reqwest::Client::new(), server.uri());
    let listing = client.franchise_relations("9").await.expect("listing");
    assert_eq!(listing.relations.len(), 25);
    assert!(listing.complete);
}

/// A listing past the page cap is read to the cap and marked
/// incomplete.
#[tokio::test]
async fn a_listing_past_the_cap_is_incomplete() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/9/media-relationships"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page_body(1..21, 500)))
        .expect(u64::try_from(FRANCHISE_MAX_PAGES).expect("small"))
        .mount(&server)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), server.uri());
    let listing = client.franchise_relations("9").await.expect("listing");
    assert_eq!(listing.relations.len(), 20 * FRANCHISE_MAX_PAGES);
    assert!(!listing.complete);
}

/// Kitsu answering with an error status is that status, not a listing.
#[tokio::test]
async fn an_error_status_is_no_listing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/9/media-relationships"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), server.uri());
    assert!(matches!(
        client.franchise_relations("9").await,
        Err(AniError::Upstream { status: 503 })
    ));
}
