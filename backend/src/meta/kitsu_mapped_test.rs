//! Kitsu's mappings lookup for every entry mapped to one tracker id,
//! against the bodies Kitsu served for the Attack on Titan Final
//! Chapters and Tower of God's second season.

use super::*;
use wiremock::matchers::{method, path, query_param};

const FINAL_CHAPTERS_MAPPED: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-mal-51535.json");
const TOWER_OF_GOD_MAPPED: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-anilist-153406.json");

#[tokio::test]
async fn every_kitsu_entry_mapped_to_a_tracker_id_is_returned_with_its_count_and_dates() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalSite]", "myanimelist/anime"))
        .and(query_param("filter[externalId]", "51535"))
        .and(query_param("include", "item"))
        .and(query_param("fields[anime]", ANIME_FIELDS))
        .and(query_param("page[limit]", "20"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(FINAL_CHAPTERS_MAPPED))
        .expect(1)
        .mount(&server)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), server.uri());
    let got = client
        .anime_by_external_id("myanimelist/anime", 51_535)
        .await
        .expect("ok");
    assert!(got.complete);
    assert_eq!(
        got.entries
            .iter()
            .map(|a| (a.id.as_str(), a.episode_count, a.start_date.as_deref()))
            .collect::<Vec<_>>(),
        [
            ("46038", Some(1), Some("2023-03-03")),
            ("47132", Some(1), Some("2023-11-04")),
        ]
    );
}

#[tokio::test]
async fn one_mapped_entry_and_none_read_as_they_are() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(query_param("filter[externalId]", "153406"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(TOWER_OF_GOD_MAPPED))
        .mount(&server)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(query_param("filter[externalId]", "1"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":[],"meta":{"count":0}}"#),
        )
        .mount(&server)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), server.uri());
    let one = client
        .anime_by_external_id("anilist/anime", 153_406)
        .await
        .expect("ok");
    assert_eq!(one.entries.len(), 1);
    assert_eq!(one.entries[0].id, "46312");
    assert_eq!(one.entries[0].episode_count, Some(13));
    let none = client
        .anime_by_external_id("anilist/anime", 1)
        .await
        .expect("ok");
    assert!(none.entries.is_empty());
    assert!(none.complete);
}

#[tokio::test]
async fn an_upstream_failure_and_a_body_that_is_not_json_api_are_reported() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), server.uri());
    assert!(matches!(
        client
            .anime_by_external_id("anilist/anime", 5)
            .await
            .unwrap_err(),
        AniError::Upstream { status: 503 }
    ));
    assert!(matches!(
        parse_mapped_anime_response(b"nope").unwrap_err(),
        AniError::ParseFailed { .. }
    ));
}

#[test]
fn a_page_holding_fewer_mappings_than_kitsu_counts_is_not_complete() {
    let body = br#"{"data":[{"id":"m1","type":"mappings","relationships":{"item":{"data":{"type":"anime","id":"1"}}}}],
        "included":[{"id":"1","type":"anime","attributes":{"canonicalTitle":"One","episodeCount":12}}],
        "meta":{"count":21}}"#;
    let got = parse_mapped_anime_response(body).expect("parsed");
    assert_eq!(got.entries.len(), 1);
    assert!(!got.complete);
    // No count at all says nothing of what is past the page.
    let bare = br#"{"data":[],"included":[]}"#;
    assert!(!parse_mapped_anime_response(bare).expect("parsed").complete);
}

#[test]
fn a_mapping_whose_anime_cannot_be_read_is_no_answer() {
    // A mapping names Kitsu's 2, which the page does not include, or
    // includes with attributes that do not read. The page cannot say
    // who owns the entry, so it is no answer, not an empty set.
    for included in [
        r#"[]"#,
        r#"[{"id":"2","type":"anime","attributes":{"episodeCount":"many"}}]"#,
    ] {
        let body = format!(
            r#"{{"data":[{{"id":"m1","type":"mappings","relationships":{{"item":{{"data":{{"type":"anime","id":"2"}}}}}}}}],
                "included":{included},"meta":{{"count":1}}}}"#
        );
        assert!(
            matches!(
                parse_mapped_anime_response(body.as_bytes()),
                Err(AniError::ParseFailed { .. })
            ),
            "{included}"
        );
    }
}

#[test]
fn a_mapping_with_no_item_is_no_answer_and_a_null_item_maps_to_nothing() {
    for mapping in [
        r#"{"id":"m1"}"#,
        r#"{"id":"m1","relationships":null}"#,
        r#"{"id":"m1","relationships":{"item":null}}"#,
    ] {
        let body = format!(r#"{{"data":[{mapping}],"included":[],"meta":{{"count":1}}}}"#);
        assert!(
            matches!(
                parse_mapped_anime_response(body.as_bytes()),
                Err(AniError::ParseFailed { .. })
            ),
            "{mapping}"
        );
    }
    let body = br#"{"data":[{"id":"m1","relationships":{"item":{"data":null}}}],"included":[],"meta":{"count":1}}"#;
    let got = parse_mapped_anime_response(body).expect("an answer");
    assert!(got.entries.is_empty());
    assert!(got.complete);
}
