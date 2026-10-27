//! MAL's `media_evidence`: what the episode map reads about an anime —
//! its total, status, run and sequels — from one `/anime/{id}` read,
//! against the bodies MAL served for Ranma ½, the Attack on Titan Final
//! Chapters and PriPara.

use super::*;
use wiremock::matchers::{header, method, path, query_param};

const RANMA: &str = include_str!("../../../tests/fixtures/tracker-map/wire/mal-anime-210.json");
const FINAL_CHAPTERS: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/mal-anime-51535.json");
const PRIPARA: &str = include_str!("../../../tests/fixtures/tracker-map/wire/mal-anime-23135.json");

fn provider(api_uri: &str) -> MalProvider {
    MalProvider::with_bases(
        reqwest::Client::new(),
        api_uri.to_string(),
        "http://unused-token".to_string(),
        MalRefreshState::new(),
    )
}

fn tokens() -> Tokens {
    Tokens {
        access_token: "mal-access".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

async fn evidence_for(id: u32, body: &'static str) -> MediaEvidence {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path(format!("/anime/{id}")))
        .and(header("authorization", "Bearer mal-access"))
        .and(query_param(
            "fields",
            "num_episodes,status,media_type,start_date,end_date,related_anime",
        ))
        .and(query_param("nsfw", "true"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(&server)
        .await;
    provider(&server.uri())
        .media_evidence(&tokens(), ProviderMediaId(id))
        .await
        .expect("media_evidence ok")
}

#[tokio::test]
async fn ranma_s_entry_reads_its_total_run_and_sequel() {
    let got = evidence_for(210, RANMA).await;
    assert_eq!(
        got.show,
        crate::account::episode_map::TrackerShow {
            id: 210,
            id_mal: None,
            episodes: Some(161),
            finished: true,
            start: Some("1989-04-15".into()),
            end: Some("1992-09-25".into()),
        }
    );
    assert!(got.series);
    assert_eq!(got.sequels, [ProviderMediaId(1007)]);
    assert_eq!(got.listed, None);
}

#[tokio::test]
async fn a_tv_special_is_a_series_and_a_prequel_is_no_sequel() {
    let got = evidence_for(51_535, FINAL_CHAPTERS).await;
    assert_eq!(got.show.episodes, Some(2));
    assert!(got.show.finished);
    assert!(got.series);
    assert!(got.sequels.is_empty(), "{:?}", got.sequels);
}

#[tokio::test]
async fn pripara_s_sequel_is_read_from_related_anime() {
    let got = evidence_for(23_135, PRIPARA).await;
    assert_eq!(got.show.episodes, Some(140));
    assert_eq!(got.sequels, [ProviderMediaId(34_787)]);
}

#[tokio::test]
async fn a_movie_an_airing_show_and_a_bare_year_read_as_they_are() {
    let crafted = r#"{"id":9,"title":"x","num_episodes":0,"status":"currently_airing","media_type":"movie",
        "start_date":"2024","related_anime":[{"node":{"id":10,"title":"y"},"relation_type":"sequel"},
        {"node":{"id":11,"title":"z"},"relation_type":"sequel"}]}"#;
    let got = evidence_for(9, crafted).await;
    assert_eq!(got.show.episodes, None);
    assert!(!got.show.finished);
    assert!(!got.series);
    assert_eq!(got.show.start, Some("2024".into()));
    assert_eq!(got.show.end, None);
    assert_eq!(got.sequels, [ProviderMediaId(10), ProviderMediaId(11)]);
}

#[tokio::test]
async fn a_body_without_an_id_is_a_parse_failure() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("[]"))
        .mount(&server)
        .await;
    let err = provider(&server.uri())
        .media_evidence(&tokens(), ProviderMediaId(9))
        .await
        .expect_err("not an anime");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

#[test]
fn an_anime_without_its_related_anime_is_no_answer() {
    // The read asks for `related_anime`. A body that leaves it out, or
    // a row that leaves out its relation, says nothing of the sequels;
    // read as none, they would end the chain at an anime that has one.
    for body in [
        r#"{"id":9,"num_episodes":12,"status":"finished_airing","media_type":"tv"}"#,
        r#"{"id":9,"num_episodes":12,"related_anime":null}"#,
        r#"{"id":9,"num_episodes":12,"related_anime":[{"node":{"id":10}}]}"#,
    ] {
        assert!(
            matches!(
                evidence::parse_media_evidence(body.as_bytes()),
                Err(AniError::ParseFailed { .. })
            ),
            "{body}"
        );
    }
    // An empty list is an answer: no sequels.
    let got = evidence::parse_media_evidence(
        br#"{"id":9,"num_episodes":12,"status":"finished_airing","media_type":"tv","related_anime":[]}"#,
    )
        .expect("an answer");
    assert!(got.sequels.is_empty());
}

#[test]
fn an_anime_that_leaves_out_its_count_status_or_type_is_no_answer() {
    // The read asks for num_episodes, status and media_type. Left out,
    // a missing status would read as not finished and a missing type as
    // a series. The dates MyAnimeList leaves out when it has none, so
    // they stay optional.
    let full = serde_json::json!({
        "id": 9, "num_episodes": 12, "status": "finished_airing", "media_type": "tv",
        "related_anime": []
    });
    for field in ["num_episodes", "status", "media_type"] {
        let mut body = full.clone();
        body.as_object_mut().expect("object").remove(field);
        assert!(
            matches!(
                evidence::parse_media_evidence(body.to_string().as_bytes()),
                Err(AniError::ParseFailed { .. })
            ),
            "{field}"
        );
    }
    assert!(evidence::parse_media_evidence(full.to_string().as_bytes()).is_ok());
}

#[test]
fn an_anime_with_no_relations_is_sent_with_an_empty_related_anime() {
    // Perfect Blue, read with the evidence fields: MyAnimeList answers
    // a show with no related anime with `related_anime: []`, not by
    // leaving the key out, so the reader's refusal of a missing key
    // never refuses a real show with no relations.
    let body = include_str!("../../../tests/fixtures/tracker-map/wire/mal-anime-437.json");
    let got = evidence::parse_media_evidence(body.as_bytes()).expect("an answer");
    assert!(got.sequels.is_empty());
    assert!(!got.series);
    assert_eq!(got.show.episodes, Some(1));
    assert!(got.show.finished);
}
