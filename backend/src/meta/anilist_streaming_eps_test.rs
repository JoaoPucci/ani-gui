use super::*;

#[test]
fn parse_streaming_episodes_response_extracts_ep_num_and_url() {
    // Real shape from a live Gintama probe (MAL 918) — three eps,
    // all with the standard "Episode N - ..." title format and a
    // Crunchyroll CDN thumbnail.
    let body = br##"{
        "data": {
            "Media": {
                "streamingEpisodes": [
                    {
                        "title": "Episode 1 - You Guys!! Do You Even Have a Gintama? (Part 1)",
                        "thumbnail": "https://img1.ak.crunchyroll.com/i/spire1-tmb/abc_full.jpg"
                    },
                    {
                        "title": "Episode 2 - You Guys!! Do You Even Have a Gintama? (Part 2)",
                        "thumbnail": "https://img1.ak.crunchyroll.com/i/spire3-tmb/def_full.jpg"
                    },
                    {
                        "title": "Episode 3 - Nobody with Naturally Wavy Hair Can Be That Bad",
                        "thumbnail": "https://img1.ak.crunchyroll.com/i/spire2-tmb/ghi_full.jpg"
                    }
                ]
            }
        }
    }"##;
    let got = parse_streaming_episodes_response(body).expect("parses");
    assert_eq!(got.len(), 3);
    assert_eq!(got[0].0, 1);
    assert!(got[0].1.contains("abc_full.jpg"));
    assert_eq!(got[1].0, 2);
    assert_eq!(got[2].0, 3);
}

#[test]
fn parse_streaming_episodes_response_drops_entries_without_thumbnail() {
    // AniList sometimes lists an entry with title but null thumbnail
    // (the listing got scraped before Crunchyroll uploaded the still).
    // Those entries are useless for backfill — skip them entirely.
    let body = br##"{
        "data": {
            "Media": {
                "streamingEpisodes": [
                    { "title": "Episode 1 - ...", "thumbnail": "https://x.cdn/1.jpg" },
                    { "title": "Episode 2 - ...", "thumbnail": null },
                    { "title": "Episode 3 - ...", "thumbnail": "https://x.cdn/3.jpg" }
                ]
            }
        }
    }"##;
    let got = parse_streaming_episodes_response(body).expect("parses");
    let nums: Vec<u32> = got.iter().map(|(n, _)| *n).collect();
    assert_eq!(nums, vec![1, 3]);
}

#[test]
fn parse_streaming_episodes_response_drops_titles_without_episode_prefix() {
    // Some entries are OVAs / specials / movies with titles like
    // "OVA 1 - …" or "Special - …". Kitsu's `number` field is for
    // numbered TV episodes; merging those would be nonsense. Drop
    // any title we can't extract an integer episode number from.
    let body = br##"{
        "data": {
            "Media": {
                "streamingEpisodes": [
                    { "title": "Episode 1 - Real", "thumbnail": "https://x.cdn/1.jpg" },
                    { "title": "OVA 1 - Special", "thumbnail": "https://x.cdn/o.jpg" },
                    { "title": "Movie", "thumbnail": "https://x.cdn/m.jpg" },
                    { "title": "Episode 2 - Real", "thumbnail": "https://x.cdn/2.jpg" }
                ]
            }
        }
    }"##;
    let got = parse_streaming_episodes_response(body).expect("parses");
    let nums: Vec<u32> = got.iter().map(|(n, _)| *n).collect();
    assert_eq!(nums, vec![1, 2]);
}

#[test]
fn parse_streaming_episodes_response_drops_half_episode_decimals() {
    // Recap eps like "Episode 1061.5" exist in long-runners. Kitsu's
    // `number` is u32, so half-eps can't merge — drop them. Matches
    // the half-episode handling already in `meta::kitsu` for the
    // integer-cap derivation (see `max_integer_episode`).
    let body = br##"{
        "data": {
            "Media": {
                "streamingEpisodes": [
                    { "title": "Episode 1 - Real", "thumbnail": "https://x.cdn/1.jpg" },
                    { "title": "Episode 1.5 - Recap", "thumbnail": "https://x.cdn/r.jpg" },
                    { "title": "Episode 2 - Real", "thumbnail": "https://x.cdn/2.jpg" }
                ]
            }
        }
    }"##;
    let got = parse_streaming_episodes_response(body).expect("parses");
    let nums: Vec<u32> = got.iter().map(|(n, _)| *n).collect();
    assert_eq!(nums, vec![1, 2]);
}

#[test]
fn parse_streaming_episodes_response_returns_empty_when_media_is_null() {
    // AniList answers Media: null when no media matches the MAL id.
    // Same shape as banner_for_mal_id's unmapped case — empty result,
    // not an error.
    let body = br#"{"data":{"Media":null}}"#;
    let got = parse_streaming_episodes_response(body).expect("parses");
    assert!(got.is_empty());
}

#[test]
fn parse_streaming_episodes_response_returns_empty_when_field_is_null() {
    // Media present but streamingEpisodes itself is null — happens
    // for shows AniList catalogues without any Crunchyroll listing.
    let body = br#"{"data":{"Media":{"streamingEpisodes":null}}}"#;
    let got = parse_streaming_episodes_response(body).expect("parses");
    assert!(got.is_empty());
}

#[test]
fn parse_streaming_episodes_response_rejects_non_envelope_payload() {
    let body = br#"<html>error</html>"#;
    let err = parse_streaming_episodes_response(body).unwrap_err();
    assert!(matches!(err, AniError::ParseFailed { .. }));
}

#[tokio::test]
async fn streaming_episodes_for_mal_id_posts_correct_query() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "query": STREAMING_EPS_BY_MAL_GQL,
            "variables": { "idMal": 918 },
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r##"{"data":{"Media":{"streamingEpisodes":[{"title":"Episode 1 - …","thumbnail":"https://x.cdn/1.jpg"}]}}}"##,
        ))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = streaming_episodes_for_mal_id(&client, 918, Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, 1);
}

#[tokio::test]
async fn streaming_episodes_for_mal_id_propagates_upstream_5xx() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(502))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let err = streaming_episodes_for_mal_id(&client, 918, Some(&server.uri()))
        .await
        .unwrap_err();
    assert!(matches!(err, AniError::Upstream { status: 502 }));
}

#[tokio::test]
async fn streaming_eps_map_for_mal_id_dedups_first_wins() {
    // Live AniList sometimes lists multiple streamingEpisodes for
    // the same ep number when a show has subbed + dubbed tracks
    // both registered with Crunchyroll. The pair list preserves
    // order; the map wrapper collapses to first-wins so the
    // backfill caller doesn't have to.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r##"{"data":{"Media":{"streamingEpisodes":[
                {"title":"Episode 1 - sub","thumbnail":"https://x.cdn/1-sub.jpg"},
                {"title":"Episode 1 - dub","thumbnail":"https://x.cdn/1-dub.jpg"},
                {"title":"Episode 2 - sub","thumbnail":"https://x.cdn/2.jpg"}
            ]}}}"##,
        ))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = streaming_eps_map_for_mal_id(&client, 918, Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.len(), 2);
    assert!(got[&1].contains("1-sub"));
    assert!(got[&2].contains("2.jpg"));
}

#[tokio::test]
async fn streaming_eps_map_for_mal_id_propagates_upstream_5xx() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let err = streaming_eps_map_for_mal_id(&client, 918, Some(&server.uri()))
        .await
        .unwrap_err();
    assert!(matches!(err, AniError::Upstream { status: 503 }));
}

#[tokio::test]
async fn streaming_episodes_for_mal_id_returns_empty_when_media_unmapped() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(404)
                .set_body_string(crate::meta::anilist_media::ANILIST_NOT_FOUND_BODY),
        )
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = streaming_episodes_for_mal_id(&client, 99_999_999, Some(&server.uri()))
        .await
        .expect("ok");
    assert!(got.is_empty());
}

#[tokio::test]
async fn streaming_eps_map_for_media_by_anilist_id_queries_media_by_id() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("Media(id: $id"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "variables": { "id": 207_141 },
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Media":{"streamingEpisodes":[{"title":"Episode 2 - Two","thumbnail":"https://x.cdn/2.jpg"}]}}}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = streaming_eps_map_for_media(&client, MediaRef::AniList(207_141), Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.get(&2).map(String::as_str), Some("https://x.cdn/2.jpg"));
}

#[tokio::test]
async fn streaming_eps_map_for_media_by_mal_id_keeps_the_idmal_query() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "query": STREAMING_EPS_BY_MAL_GQL,
            "variables": { "idMal": 918 },
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Media":{"streamingEpisodes":[{"title":"Episode 1 - One","thumbnail":"https://x.cdn/1.jpg"}]}}}"#,
        ))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = streaming_eps_map_for_media(&client, MediaRef::Mal(918), Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.get(&1).map(String::as_str), Some("https://x.cdn/1.jpg"));
}

async fn mount_media(
    server: &wiremock::MockServer,
    variables: serde_json::Value,
    body: &str,
    times: u64,
) {
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_partial_json(
            serde_json::json!({ "variables": variables }),
        ))
        .respond_with(
            // An `errors` body is AniList's not-found answer, sent as 404.
            wiremock::ResponseTemplate::new(if body.contains("\"errors\"") {
                404
            } else {
                200
            })
            .set_body_string(body.to_string()),
        )
        .expect(times)
        .mount(server)
        .await;
}

#[tokio::test]
async fn streaming_eps_map_for_ids_retries_by_anilist_id_when_anilist_lacks_the_mal_id() {
    let server = wiremock::MockServer::start().await;
    mount_media(
        &server,
        serde_json::json!({ "idMal": 21 }),
        crate::meta::anilist_media::ANILIST_NOT_FOUND_BODY,
        1,
    )
    .await;
    mount_media(
        &server,
        serde_json::json!({ "id": 30 }),
        r#"{"data":{"Media":{"streamingEpisodes":[{"title":"Episode 3 - Three","thumbnail":"https://x.cdn/3.jpg"}]}}}"#,
        1,
    )
    .await;
    let client = reqwest::Client::new();
    let got = streaming_eps_map_for_ids(&client, Some(21), Some(30), Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.get(&3).map(String::as_str), Some("https://x.cdn/3.jpg"));
}

#[tokio::test]
async fn streaming_eps_map_for_ids_does_not_retry_a_media_without_episodes() {
    let server = wiremock::MockServer::start().await;
    mount_media(
        &server,
        serde_json::json!({ "idMal": 21 }),
        r#"{"data":{"Media":{"streamingEpisodes":[]}}}"#,
        1,
    )
    .await;
    mount_media(
        &server,
        serde_json::json!({ "id": 30 }),
        crate::meta::anilist_media::ANILIST_NOT_FOUND_BODY,
        0,
    )
    .await;
    let client = reqwest::Client::new();
    let got = streaming_eps_map_for_ids(&client, Some(21), Some(30), Some(&server.uri()))
        .await
        .expect("ok");
    assert!(got.is_empty());
}

proptest::proptest! {
    /// Every episode number AniList listed keeps the URL it was listed
    /// with first, and nothing else appears. Episode numbers come from
    /// a small range so duplicates are the common case.
    #[test]
    fn dedup_keeps_each_episodes_first_url(
        pairs in proptest::collection::vec((0u32..12, "[a-z]{0,6}"), 0..40),
    ) {
        let got = dedup_first_wins(pairs.clone());
        let mut want = HashMap::new();
        for (n, url) in &pairs {
            if !want.contains_key(n) {
                want.insert(*n, url.clone());
            }
        }
        proptest::prop_assert_eq!(got, want);
    }
}
