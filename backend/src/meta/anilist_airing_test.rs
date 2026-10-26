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

// --- batch fetch ----------------------------------------------------
// One Page(media(id_in: [...])) request answers airing for a whole
// rail; per-show requests were the app's most frequent AniList
// caller and the main rate-limit burst source at launch.

const BATCH_BODY: &str = r#"{"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[
    {"id":207141,"status":"RELEASING","episodes":12,
     "nextAiringEpisode":{"episode":3,"airingAt":1784215800},
     "airingSchedule":{"nodes":[{"episode":3,"airingAt":1784215800}]}},
    {"id":185874,"status":"NOT_YET_RELEASED","episodes":13,
     "nextAiringEpisode":{"episode":1,"airingAt":1784988000},
     "airingSchedule":{"nodes":[{"episode":1,"airingAt":1784988000},{"episode":2,"airingAt":1785592800}]}}
]}}}"#;

#[test]
fn parse_airing_batch_maps_each_media_by_id() {
    let got = parse_airing_batch_response(BATCH_BODY.as_bytes()).expect("parses");
    assert_eq!(got.len(), 2);
    let releasing = &got[&207141];
    assert_eq!(releasing.aired, Some(2));
    assert_eq!(releasing.next_episode, Some(3));
    let unreleased = &got[&185874];
    assert_eq!(unreleased.aired, Some(0));
    assert_eq!(unreleased.next_airing_at, Some(1_784_988_000));
    assert_eq!(unreleased.upcoming.len(), 2);
}

#[test]
fn parse_airing_batch_tolerates_an_empty_page() {
    let got = parse_airing_batch_response(
        br#"{"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[]}}}"#,
    )
    .expect("parses");
    assert!(got.is_empty());
}

#[tokio::test]
async fn airing_status_batch_answers_many_ids_with_one_request() {
    use wiremock::matchers::method;
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(BATCH_BODY))
        .expect(1)
        .mount(&server)
        .await;
    let got = airing_status_batch(
        &reqwest::Client::new(),
        &[207141, 185874],
        Some(&server.uri()),
    )
    .await
    .expect("ok");
    assert_eq!(got.len(), 2);
    // MockServer verifies expect(1) on drop — one request, two answers.
}

#[tokio::test]
async fn airing_status_batch_skips_the_network_for_no_ids() {
    let server = wiremock::MockServer::start().await;
    // No mounts; any request would be recorded.
    let got = airing_status_batch(&reqwest::Client::new(), &[], Some(&server.uri()))
        .await
        .expect("ok");
    assert!(got.is_empty());
    assert!(server
        .received_requests()
        .await
        .expect("recorded")
        .is_empty());
}

#[tokio::test]
async fn airing_status_is_none_for_anilists_not_found_reply() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(404)
                .set_body_string(crate::meta::anilist_media::ANILIST_NOT_FOUND_BODY),
        )
        .expect(1)
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = airing_status(&client, None, Some(99_999_999), Some(&server.uri()))
        .await
        .expect("absence is not an upstream failure");
    assert!(got.is_none());
}

/// A by-MAL batch page listing `media` as `(idMal, status, episodes)`,
/// with `cut` as its `hasNextPage`.
fn by_mal_page(media: &[(u32, &str, u32)], cut: bool) -> String {
    let nodes: Vec<String> = media
        .iter()
        .map(|(mal, status, eps)| {
            format!(
                r#"{{"idMal":{mal},"status":"{status}","episodes":{eps},"nextAiringEpisode":null,"airingSchedule":{{"nodes":[]}}}}"#
            )
        })
        .collect();
    format!(
        r#"{{"data":{{"Page":{{"pageInfo":{{"hasNextPage":{cut}}},"media":[{}]}}}}}}"#,
        nodes.join(",")
    )
}

#[tokio::test]
async fn a_mal_id_several_media_record_is_seeded_as_the_single_lookup_answers() {
    // AniList 137728 (finished, 52) and 196613 (not yet released)
    // both record MAL 51039. The batch lists both; the single airing
    // lookup by MAL id, which the detail page makes, answers 137728.
    // The batch takes that answer, not whichever node came last.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("AiringBatchByMal"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(by_mal_page(
                &[
                    (51039, "FINISHED", 52),
                    (51039, "NOT_YET_RELEASED", 62),
                    (7, "FINISHED", 12),
                ],
                false,
            )),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("query Airing("))
        .and(wiremock::matchers::body_string_contains("\"idMal\":51039"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Media":{"status":"FINISHED","episodes":52,"nextAiringEpisode":null,"airingSchedule":{"nodes":[]}}}}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    let got = airing_status_batch_by_mal(&reqwest::Client::new(), &[51039, 7], Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.get(&51039).and_then(|s| s.aired), Some(52));
    assert_eq!(got.get(&7).and_then(|s| s.aired), Some(12));
}

#[tokio::test]
async fn a_by_mal_batch_whose_page_was_cut_is_split_until_every_page_is_whole() {
    let server = wiremock::MockServer::start().await;
    for (ids, page) in [
        ("[1,2]", by_mal_page(&[(1, "FINISHED", 10)], true)),
        ("[1]", by_mal_page(&[(1, "FINISHED", 10)], false)),
        ("[2]", by_mal_page(&[(2, "FINISHED", 20)], false)),
    ] {
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_string_contains(ids))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(page))
            .mount(&server)
            .await;
    }
    let got = airing_status_batch_by_mal(&reqwest::Client::new(), &[1, 2], Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.get(&1).and_then(|s| s.aired), Some(10));
    assert_eq!(got.get(&2).and_then(|s| s.aired), Some(20));
}

#[tokio::test]
async fn a_failed_single_lookup_leaves_only_its_mal_id_out_of_the_batch() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("AiringBatchByMal"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(by_mal_page(
                &[
                    (51039, "FINISHED", 52),
                    (51039, "NOT_YET_RELEASED", 62),
                    (7, "FINISHED", 12),
                ],
                false,
            )),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("query Airing("))
        .respond_with(wiremock::ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let got = airing_status_batch_by_mal(&reqwest::Client::new(), &[51039, 7], Some(&server.uri()))
        .await
        .expect("the batch answered");
    assert_eq!(got.get(&51039), None);
    assert_eq!(got.get(&7).and_then(|s| s.aired), Some(12));
}

#[tokio::test]
async fn one_mal_id_whose_own_page_is_cut_is_seeded_from_the_single_lookup() {
    // More media record MAL 1 than a page holds: its page is cut even
    // alone, so the single lookup answers it. One the single lookup
    // does not index is left out.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("AiringBatchByMal"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(by_mal_page(&[(1, "FINISHED", 10)], true)),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("query Airing("))
        .and(wiremock::matchers::body_string_contains("\"idMal\":1"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Media":{"status":"FINISHED","episodes":10,"nextAiringEpisode":null,"airingSchedule":{"nodes":[]}}}}"#,
        ))
        .mount(&server)
        .await;
    let got = airing_status_batch_by_mal(&reqwest::Client::new(), &[1], Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.get(&1).and_then(|s| s.aired), Some(10));

    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("AiringBatchByMal"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(by_mal_page(&[(1, "FINISHED", 10)], true)),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("query Airing("))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#),
        )
        .mount(&server)
        .await;
    let got = airing_status_batch_by_mal(&reqwest::Client::new(), &[1], Some(&server.uri()))
        .await
        .expect("ok");
    assert!(got.is_empty());
}

#[tokio::test]
async fn an_id_keyed_batch_whose_page_is_cut_asks_the_single_lookup_by_anilist_id() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("AiringBatch("))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"data":{"Page":{"pageInfo":{"hasNextPage":true},"media":[]}}}"#,
            ),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("query Airing("))
        .and(wiremock::matchers::body_string_contains("\"id\":207141"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Media":{"status":"FINISHED","episodes":12,"nextAiringEpisode":null,"airingSchedule":{"nodes":[]}}}}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    let got = airing_status_batch(&reqwest::Client::new(), &[207141], Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.get(&207141).and_then(|s| s.aired), Some(12));
}

#[test]
fn a_batch_page_missing_what_was_asked_for_is_no_answer() {
    // The batch asks for pageInfo { hasNextPage }, the media list, and
    // each node's key. A page that leaves one out cannot say whether it
    // holds every show, or whose schedule a node is.
    for body in [
        r#"{"data":{"Page":{"media":[]}}}"#,
        r#"{"data":{"Page":{"pageInfo":null,"media":[]}}}"#,
        r#"{"data":{"Page":{"pageInfo":{},"media":[]}}}"#,
        r#"{"data":{"Page":{"pageInfo":{"hasNextPage":false}}}}"#,
        r#"{"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[{"status":"FINISHED","episodes":12}]}}}"#,
    ] {
        assert!(
            matches!(
                parse_airing_batch_response(body.as_bytes()),
                Err(AniError::ParseFailed { .. })
            ),
            "{body}"
        );
    }
    // A node whose key is null names no show, and is skipped.
    let got = parse_airing_batch_response(
        br#"{"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[{"id":null,"status":"FINISHED","episodes":12,"nextAiringEpisode":null,"airingSchedule":null}]}}}"#,
    )
    .expect("an answer");
    assert!(got.is_empty());
}

#[test]
fn a_batch_node_missing_a_field_it_was_asked_for_is_no_answer() {
    // Each node is asked for its status, episodes, next airing and
    // schedule. A node that leaves one out cannot say how far the show
    // has aired; null keeps its meaning (nothing scheduled, unknown).
    for field in ["status", "episodes", "nextAiringEpisode", "airingSchedule"] {
        let mut node = serde_json::json!({
            "id": 5,
            "status": "RELEASING",
            "episodes": 12,
            "nextAiringEpisode": null,
            "airingSchedule": { "nodes": [] },
        });
        node.as_object_mut().expect("object").remove(field);
        let body = serde_json::json!({
            "data": { "Page": { "pageInfo": { "hasNextPage": false }, "media": [node] } }
        });
        assert!(
            matches!(
                parse_airing_batch_response(body.to_string().as_bytes()),
                Err(AniError::ParseFailed { .. })
            ),
            "{field}"
        );
    }
    let nulls = br#"{"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[{"id":5,"status":null,"episodes":null,"nextAiringEpisode":null,"airingSchedule":null}]}}}"#;
    assert_eq!(
        parse_airing_batch_response(nulls).expect("an answer").len(),
        1
    );
}
