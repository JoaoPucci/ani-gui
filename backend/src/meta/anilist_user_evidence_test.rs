//! AniList's `media_evidence`: what the episode map reads about an
//! entry — its total, status, run, series sequels and the numbers its
//! `streamingEpisodes` carry — from one `Media` query, against the
//! bodies AniList served for Ranma ½, Nettou-hen, Haruhi 2009 and
//! Unlimited Blade Works.

use super::*;
use wiremock::matchers::{body_string_contains, header, method};

const RANMA: &str = include_str!("../../../tests/fixtures/tracker-map/wire/anilist-media-210.json");
const NETTOU_HEN: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/anilist-media-149939.json");
const HARUHI: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/anilist-media-4382.json");
const UBW: &str = include_str!("../../../tests/fixtures/tracker-map/wire/anilist-media-19603.json");

async fn evidence_for(id: u32, body: &'static str) -> MediaEvidence {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .and(header("authorization", "Bearer test-access-token"))
        .and(body_string_contains("relations"))
        .and(body_string_contains("streamingEpisodes"))
        .and(body_string_contains(format!("\"mediaId\":{id}")))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(&server)
        .await;
    let provider = AniListProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_string(),
    );
    let tokens = Tokens {
        access_token: "test-access-token".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    provider
        .media_evidence(&tokens, ProviderMediaId(id))
        .await
        .expect("media_evidence ok")
}

#[tokio::test]
async fn ranma_s_entry_reads_its_total_run_and_series_sequel() {
    let got = evidence_for(210, RANMA).await;
    assert_eq!(
        got.show,
        crate::account::episode_map::TrackerShow {
            id: 210,
            id_mal: Some(210),
            episodes: Some(18),
            finished: true,
            start: Some("1989-04-15".into()),
            end: Some("1989-09-16".into()),
        }
    );
    assert!(got.series);
    assert_eq!(got.sequels, [ProviderMediaId(149_939)]);
    // AniList lists no episodes for it.
    assert_eq!(got.listed, None);
}

#[tokio::test]
async fn nettou_hen_has_no_mal_id_and_its_ova_sequel_is_a_series_sequel() {
    let got = evidence_for(149_939, NETTOU_HEN).await;
    assert_eq!(got.show.id_mal, None);
    assert_eq!(got.show.episodes, Some(143));
    // Its PREQUEL edge (210) is not a sequel; its OVA sequel is one.
    assert_eq!(got.sequels, [ProviderMediaId(1007)]);
}

#[tokio::test]
async fn a_movie_is_not_a_series_sequel_and_the_list_s_numbers_are_kept() {
    let got = evidence_for(4382, HARUHI).await;
    assert_eq!(got.show.episodes, Some(28));
    assert!(
        got.sequels.is_empty(),
        "the movie sequel is not one: {:?}",
        got.sequels
    );
    let listed = got.listed.expect("28 listed episodes");
    assert_eq!(listed.len(), 28);
    assert_eq!(listed[0], "1");
    assert_eq!(listed[27], "28");
}

#[tokio::test]
async fn a_prologue_and_a_half_episode_keep_their_numbers() {
    let got = evidence_for(19_603, UBW).await;
    assert_eq!(got.show.id_mal, Some(22_297));
    let listed = got.listed.expect("listed");
    assert_eq!(listed[0], "0");
    assert_eq!(listed[1], "1");
    assert!(listed.contains(&"25".to_string()));
    // A title with no "Episode N" is not a number.
    let crafted = r#"{"data":{"Media":{"id":5,"idMal":null,"episodes":2,"status":"RELEASING","format":"MOVIE",
        "startDate":{"year":2024,"month":4,"day":null},"endDate":{"year":null,"month":null,"day":null},
        "relations":{"edges":[{"relationType":"SEQUEL","node":{"id":6,"type":"MANGA","format":"MANGA"}},
                              {"relationType":"SIDE_STORY","node":{"id":7,"type":"ANIME","format":"TV"}}]},
        "streamingEpisodes":[{"title":"Episode 6.5 - Ekwari"},{"title":"Robot Carnival (Dub)"},{"title":"episode 7 - Sansa"}]}}}"#;
    let got = evidence_for(5, crafted).await;
    assert_eq!(got.show.start, Some("2024-04".into()));
    assert_eq!(got.show.end, None);
    assert!(!got.show.finished);
    assert!(!got.series);
    assert!(
        got.sequels.is_empty(),
        "a manga is no sequel: {:?}",
        got.sequels
    );
    assert_eq!(got.listed, Some(vec!["6.5".to_string(), "7".to_string()]));
}

#[tokio::test]
async fn an_unknown_total_and_an_empty_list_read_as_none() {
    let crafted = r#"{"data":{"Media":{"id":5,"idMal":5,"episodes":null,"status":"NOT_YET_RELEASED","format":"TV",
        "startDate":{"year":null},"endDate":null,"relations":{"edges":[]},"streamingEpisodes":[]}}}"#;
    let got = evidence_for(5, crafted).await;
    assert_eq!(got.show.episodes, None);
    assert_eq!(got.show.start, None);
    assert_eq!(got.listed, None);
    assert!(got.series);
}

#[tokio::test]
async fn a_body_that_is_not_the_media_envelope_is_a_parse_failure() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(r#"{"data":{}}"#))
        .mount(&server)
        .await;
    let provider = AniListProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_string(),
    );
    let tokens = Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    let err = provider
        .media_evidence(&tokens, ProviderMediaId(5))
        .await
        .expect_err("no Media");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

#[tokio::test]
async fn a_year_alone_is_kept_as_the_year_and_a_body_that_is_not_json_fails() {
    let crafted = r#"{"data":{"Media":{"id":8,"idMal":null,"episodes":4,"status":"FINISHED","format":"OVA",
        "startDate":{"year":1991,"month":null,"day":null},"endDate":{"year":1991,"month":5,"day":null},
        "relations":{"edges":[]},"streamingEpisodes":null}}}"#;
    let got = evidence_for(8, crafted).await;
    assert_eq!(got.show.start, Some("1991".into()));
    assert_eq!(got.show.end, Some("1991-05".into()));
    assert!(got.sequels.is_empty());
    assert_eq!(got.listed, None);
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("<html>"))
        .mount(&server)
        .await;
    let provider = AniListProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_string(),
    );
    let tokens = Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    let err = provider
        .media_evidence(&tokens, ProviderMediaId(8))
        .await
        .expect_err("not JSON");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

#[tokio::test]
async fn a_fuzzy_date_naming_no_real_day_or_month_is_no_date() {
    let crafted = r#"{"data":{"Media":{"id":8,"idMal":null,"episodes":4,"status":"FINISHED","format":"TV",
        "startDate":{"year":2021,"month":4,"day":31},"endDate":{"year":2021,"month":13,"day":null},
        "relations":{"edges":[]},"streamingEpisodes":null}}}"#;
    let got = evidence_for(8, crafted).await;
    assert_eq!(got.show.start, None);
    assert_eq!(got.show.end, None);
}

#[test]
fn relations_that_leave_out_what_the_query_asks_for_are_no_answer() {
    // The query asks for `relations { edges { relationType node { id
    // type format } } }`. Relations left out, or given as null, say
    // nothing of the entry's sequels; neither does a sequel whose node
    // is missing or null. Read as none, they would end the chain at an
    // entry that has a sequel.
    let envelope = |relations: &str| {
        format!(
            r#"{{"data":{{"Media":{{"id":5,"idMal":5,"episodes":12,"status":"FINISHED","format":"TV",
            "startDate":null,"endDate":null{relations},"streamingEpisodes":[]}}}}}}"#
        )
    };
    let node = r#"{"id":6,"type":"ANIME","format":"TV"}"#;
    for relations in [
        String::new(),
        r#","relations":null"#.to_string(),
        r#","relations":{}"#.to_string(),
        r#","relations":{"edges":null}"#.to_string(),
        format!(r#","relations":{{"edges":[{{"node":{node}}}]}}"#),
        r#","relations":{"edges":[{"relationType":"SEQUEL"}]}"#.to_string(),
        r#","relations":{"edges":[{"relationType":"SEQUEL","node":null}]}"#.to_string(),
        r#","relations":{"edges":[{"relationType":"SEQUEL","node":{"id":6,"format":"TV"}}]}"#
            .to_string(),
        r#","relations":{"edges":[{"relationType":"SEQUEL","node":{"id":6,"type":"ANIME"}}]}"#
            .to_string(),
    ] {
        assert!(
            matches!(
                evidence::parse_media_evidence_response(envelope(&relations).as_bytes()),
                Err(AniError::ParseFailed { .. })
            ),
            "{relations}"
        );
    }
    // A null relation type, or a null node on an edge that is not a
    // sequel, still says what it says: no sequel there.
    let got = evidence::parse_media_evidence_response(
        envelope(&format!(
            r#","relations":{{"edges":[{{"relationType":null,"node":{node}}},{{"relationType":"PREQUEL","node":null}},{{"relationType":"SEQUEL","node":{node}}}]}}"#
        ))
        .as_bytes(),
    )
    .expect("an answer");
    assert_eq!(got.sequels, [ProviderMediaId(6)]);
}

#[test]
fn a_media_that_leaves_out_a_field_the_query_asks_for_is_no_answer() {
    // idMal, episodes, status and format are asked for. Left out, a
    // missing status would read as not finished and a missing format as
    // a series; null keeps its meaning (none recorded, unknown).
    let full = serde_json::json!({
        "id": 5, "idMal": 5, "episodes": 12, "status": "FINISHED", "format": "TV",
        "startDate": null, "endDate": null,
        "relations": { "edges": [] }, "streamingEpisodes": []
    });
    for field in ["idMal", "episodes", "status", "format"] {
        let mut media = full.clone();
        media.as_object_mut().expect("object").remove(field);
        let body = serde_json::json!({ "data": { "Media": media } }).to_string();
        assert!(
            matches!(
                evidence::parse_media_evidence_response(body.as_bytes()),
                Err(AniError::ParseFailed { .. })
            ),
            "{field}"
        );
        let mut media = full.clone();
        media[field] = serde_json::Value::Null;
        let body = serde_json::json!({ "data": { "Media": media } }).to_string();
        assert!(
            evidence::parse_media_evidence_response(body.as_bytes()).is_ok(),
            "{field} null"
        );
    }
}
