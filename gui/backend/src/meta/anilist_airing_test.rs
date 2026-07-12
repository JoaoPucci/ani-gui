//! Tests for `crate::meta::anilist_airing`. Extracted via `#[path]`
//! per `project_crap_inline_test_gotcha`.

use super::*;

// `airing_status` drives the detail page's unaired-episode
// placeholders: Yani Neko (AniList 207141) announces 12 eps but only
// 2 have aired — tiles past `aired` must grey out, and the next tile
// can show its air date.
#[test]
fn parse_airing_releasing_show_derives_aired_from_next_episode() {
    let body = br#"{"data":{"Media":{
        "status":"RELEASING","episodes":12,
        "nextAiringEpisode":{"episode":3,"airingAt":1784215800}
    }}}"#;
    let got = parse_airing_response(body).expect("ok").expect("some");
    assert_eq!(
        got,
        AiringStatus {
            aired: Some(2),
            next_episode: Some(3),
            next_airing_at: Some(1_784_215_800),
            upcoming: vec![],
        }
    );
}

#[test]
fn parse_airing_finished_show_airs_the_announced_total() {
    let body = br#"{"data":{"Media":{
        "status":"FINISHED","episodes":26,"nextAiringEpisode":null
    }}}"#;
    let got = parse_airing_response(body).expect("ok").expect("some");
    assert_eq!(got.aired, Some(26));
    assert_eq!(got.next_episode, None);
    assert_eq!(got.next_airing_at, None);
}

#[test]
fn parse_airing_unreleased_show_has_zero_aired() {
    let body = br#"{"data":{"Media":{
        "status":"NOT_YET_RELEASED","episodes":null,"nextAiringEpisode":null
    }}}"#;
    let got = parse_airing_response(body).expect("ok").expect("some");
    assert_eq!(got.aired, Some(0));
}

#[test]
fn parse_airing_releasing_without_schedule_stays_ungated() {
    // No nextAiringEpisode while RELEASING (long-running shows often
    // lack schedule rows) — aired must be None so the UI doesn't hide
    // real episodes on a guess.
    let body = br#"{"data":{"Media":{
        "status":"RELEASING","episodes":null,"nextAiringEpisode":null
    }}}"#;
    let got = parse_airing_response(body).expect("ok").expect("some");
    assert_eq!(got.aired, None);
}

#[test]
fn parse_airing_null_media_is_none() {
    let got = parse_airing_response(br#"{"data":{"Media":null}}"#).expect("ok");
    assert!(got.is_none());
}

#[test]
fn parse_airing_rejects_non_envelope_payload() {
    let err = parse_airing_response(br#"<html>nope</html>"#).unwrap_err();
    assert!(matches!(err, AniError::ParseFailed { .. }));
}

#[tokio::test]
async fn airing_status_queries_by_anilist_id_when_present() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "query": AIRING_GQL,
            "variables": { "id": 207141 },
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Media":{"status":"RELEASING","episodes":12,"nextAiringEpisode":{"episode":3,"airingAt":1784215800}}}}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = airing_status(&client, Some(207141), Some(63403), Some(&server.uri()))
        .await
        .expect("ok")
        .expect("some");
    assert_eq!(got.aired, Some(2));
}

#[tokio::test]
async fn airing_status_falls_back_to_mal_id() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "query": AIRING_GQL,
            "variables": { "idMal": 21 },
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Media":{"status":"RELEASING","episodes":null,"nextAiringEpisode":null}}}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = airing_status(&client, None, Some(21), Some(&server.uri()))
        .await
        .expect("ok")
        .expect("some");
    assert_eq!(got.aired, None);
}

#[tokio::test]
async fn airing_status_without_any_id_makes_no_request() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("{}"))
        .expect(0)
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = airing_status(&client, None, None, Some(&server.uri()))
        .await
        .expect("ok");
    assert!(got.is_none());
}

// The upcoming schedule feeds per-episode dates on every unaired
// tile, not just the next one.
#[test]
fn parse_airing_extracts_the_upcoming_schedule() {
    let body = br#"{"data":{"Media":{
        "status":"RELEASING","episodes":12,
        "nextAiringEpisode":{"episode":3,"airingAt":1784215800},
        "airingSchedule":{"nodes":[
            {"episode":3,"airingAt":1784215800},
            {"episode":4,"airingAt":1784820600}
        ]}
    }}}"#;
    let got = parse_airing_response(body).expect("ok").expect("some");
    assert_eq!(
        got.upcoming,
        vec![
            UpcomingEpisode {
                episode: 3,
                airing_at: 1_784_215_800,
            },
            UpcomingEpisode {
                episode: 4,
                airing_at: 1_784_820_600,
            },
        ]
    );
}

#[test]
fn parse_airing_missing_schedule_defaults_to_empty_upcoming() {
    let body = br#"{"data":{"Media":{
        "status":"RELEASING","episodes":12,
        "nextAiringEpisode":{"episode":3,"airingAt":1784215800}
    }}}"#;
    let got = parse_airing_response(body).expect("ok").expect("some");
    assert!(got.upcoming.is_empty());
}
