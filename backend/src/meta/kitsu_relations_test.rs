//! Kitsu's two reads the episode map needs beyond an entry's own
//! mappings: every Kitsu entry mapped to one tracker id, and an entry's
//! relationships with the related anime inlined — against the bodies
//! Kitsu served for the Attack on Titan Final Chapters, Tower of God's
//! second season and Ranma ½.

use super::*;
use wiremock::matchers::{method, path, query_param};

const FINAL_CHAPTERS_MAPPED: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-mal-51535.json");
const TOWER_OF_GOD_MAPPED: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-anilist-153406.json");
const TOWER_OF_GOD_RELATIONS: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-relationships-46312.json");
const RANMA_RELATIONS: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-relationships-186.json");

#[tokio::test]
async fn every_kitsu_entry_mapped_to_a_tracker_id_is_returned_with_its_count_and_dates() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/mappings"))
        .and(query_param("filter[externalSite]", "myanimelist/anime"))
        .and(query_param("filter[externalId]", "51535"))
        .and(query_param("include", "item"))
        .and(query_param("fields[anime]", ANIME_FIELDS))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(FINAL_CHAPTERS_MAPPED))
        .expect(1)
        .mount(&server)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), server.uri());
    let got = client
        .anime_by_external_id("myanimelist/anime", 51_535)
        .await
        .expect("ok");
    assert_eq!(
        got.iter()
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
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].id, "46312");
    assert_eq!(one[0].episode_count, Some(13));
    let none = client
        .anime_by_external_id("anilist/anime", 1)
        .await
        .expect("ok");
    assert!(none.is_empty());
}

#[tokio::test]
async fn an_entry_s_relationships_inline_the_related_anime_and_drop_the_manga() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/46312/media-relationships"))
        .and(query_param("include", "destination"))
        .and(query_param("page[limit]", "20"))
        .and(query_param("fields[anime]", ANIME_FIELDS))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(TOWER_OF_GOD_RELATIONS))
        .expect(1)
        .mount(&server)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), server.uri());
    let got = client.media_relationships("46312").await.expect("ok");
    assert_eq!(
        got.iter()
            .map(|r| (r.role.as_str(), r.anime.id.as_str(), r.anime.episode_count))
            .collect::<Vec<_>>(),
        [
            ("prequel", "43004", Some(13)),
            ("sequel", "49244", Some(13))
        ]
    );
    assert_eq!(got[1].anime.start_date.as_deref(), Some("2024-10-06"));
    assert_eq!(
        got[1].anime.canonical_title,
        "Kami no Tou: Tower of God - Ouji no Kikan"
    );
}

#[tokio::test]
async fn ranma_s_only_sequel_on_kitsu_is_the_ova() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/186/media-relationships"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(RANMA_RELATIONS))
        .mount(&server)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), server.uri());
    let got = client.media_relationships("186").await.expect("ok");
    let sequels: Vec<&str> = got
        .iter()
        .filter(|r| r.role == "sequel")
        .map(|r| r.anime.id.as_str())
        .collect();
    assert_eq!(sequels, ["897"]);
    assert!(
        got.iter().all(|r| r.anime.id != "62"),
        "the manga is no relation here"
    );
    assert_eq!(got.len(), 14);
}

#[tokio::test]
async fn a_relationship_whose_destination_is_not_inlined_is_dropped() {
    let body = r#"{"data":[
        {"id":"1","type":"mediaRelationships","attributes":{"role":"sequel"},
         "relationships":{"destination":{"data":{"type":"anime","id":"9"}}}},
        {"id":"2","type":"mediaRelationships","attributes":{"role":"other"},
         "relationships":{"destination":{"data":null}}}
    ],"included":[]}"#;
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), server.uri());
    assert!(client
        .media_relationships("5")
        .await
        .expect("ok")
        .is_empty());
}

#[tokio::test]
async fn an_upstream_failure_is_reported_as_one() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), server.uri());
    assert!(matches!(
        client.media_relationships("5").await.unwrap_err(),
        AniError::Upstream { status: 503 }
    ));
    assert!(matches!(
        client
            .anime_by_external_id("anilist/anime", 5)
            .await
            .unwrap_err(),
        AniError::Upstream { status: 503 }
    ));
    let bad = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("nope"))
        .mount(&bad)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), bad.uri());
    assert!(matches!(
        client.media_relationships("5").await.unwrap_err(),
        AniError::ParseFailed { .. }
    ));
}
