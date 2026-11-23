//! One mappings answer read for its ids and its cover, and the request
//! asking Kitsu for the cover with the mappings.

use proptest::prelude::*;

use super::*;

fn body(cover: Option<serde_json::Value>, sites: &[(&str, u32)]) -> Vec<u8> {
    let mut attributes = serde_json::json!({ "canonicalTitle": "x" });
    if let Some(cover) = cover {
        attributes["coverImage"] = cover;
    }
    let included: Vec<serde_json::Value> = sites
        .iter()
        .enumerate()
        .map(|(i, (site, id))| {
            serde_json::json!({
                "id": i.to_string(),
                "type": "mappings",
                "attributes": { "externalSite": site, "externalId": id.to_string() },
            })
        })
        .collect();
    serde_json::to_vec(&serde_json::json!({
        "data": { "id": "1", "type": "anime", "attributes": attributes },
        "included": included,
    }))
    .expect("a body")
}

#[test]
fn a_cover_is_read_with_the_ids() {
    let got = parse_mappings(&body(
        Some(serde_json::json!({ "original": "https://media.kitsu.app/c.jpg" })),
        &[("myanimelist/anime", 5), ("anilist/anime", 6)],
    ))
    .expect("parsed");
    assert_eq!(
        got,
        KitsuMappings {
            ids: ExternalIds {
                mal: Some(5),
                anilist: Some(6)
            },
            has_cover: true,
        }
    );
}

#[test]
fn a_null_or_missing_cover_is_none() {
    for cover in [Some(serde_json::Value::Null), None] {
        let got = parse_mappings(&body(cover, &[("myanimelist/anime", 5)])).expect("parsed");
        assert!(!got.has_cover, "{got:?}");
        assert_eq!(got.ids.mal, Some(5));
    }
}

proptest! {
    #[test]
    fn the_ids_read_as_the_ids_alone_and_the_cover_as_whether_one_is_there(
        mal in proptest::option::of(1u32..100_000),
        anilist in proptest::option::of(1u32..100_000),
        cover in 0u8..3,
    ) {
        let mut sites = Vec::new();
        if let Some(m) = mal { sites.push(("myanimelist/anime", m)); }
        if let Some(a) = anilist { sites.push(("anilist/anime", a)); }
        let cover_value = match cover {
            0 => None,
            1 => Some(serde_json::Value::Null),
            _ => Some(serde_json::json!({ "large": "https://media.kitsu.app/c.jpg" })),
        };
        let raw = body(cover_value, &sites);
        let got = parse_mappings(&raw).expect("parsed");
        prop_assert_eq!(got.ids, ExternalIds { mal, anilist });
        prop_assert_eq!(got.ids, super::super::parse_external_ids_from_mappings(&raw).expect("parsed"));
        prop_assert_eq!(got.has_cover, cover == 2);
    }
}

#[tokio::test]
async fn the_mappings_request_asks_for_the_cover_too() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/anime/7"))
        .and(wiremock::matchers::query_param("include", "mappings"))
        .and(wiremock::matchers::query_param(
            "fields[anime]",
            "canonicalTitle,coverImage",
        ))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_bytes(body(
            Some(serde_json::json!({ "original": "https://media.kitsu.app/c.jpg" })),
            &[("myanimelist/anime", 9)],
        )))
        .mount(&server)
        .await;
    let client = super::super::KitsuClient::with_base(reqwest::Client::new(), server.uri());
    let got = client.mappings_for_kitsu_id("7").await.expect("answered");
    assert!(got.has_cover);
    assert_eq!(got.ids.mal, Some(9));
}
