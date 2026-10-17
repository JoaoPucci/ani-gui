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
    let nums: Vec<u32> = got.entries.keys().copied().collect();
    assert_eq!(nums, vec![1, 2, 3]);
    assert!(got.entries[&1]
        .thumbnail
        .as_deref()
        .is_some_and(|t| t.contains("abc_full.jpg")));
    assert_eq!(
        got.entries[&1].title,
        "You Guys!! Do You Even Have a Gintama? (Part 1)"
    );
    assert_eq!(
        got.entries[&3].title,
        "Nobody with Naturally Wavy Hair Can Be That Bad"
    );
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
    let nums: Vec<u32> = got.entries.keys().copied().collect();
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
    let nums: Vec<u32> = got.entries.keys().copied().collect();
    assert_eq!(nums, vec![1, 2]);
}

#[test]
fn parse_streaming_episodes_response_returns_empty_when_media_is_null() {
    // AniList answers Media: null when no media matches the MAL id.
    // Same shape as banner_for_mal_id's unmapped case — empty result,
    // not an error.
    let body = br#"{"data":{"Media":null}}"#;
    let got = parse_streaming_episodes_response(body).expect("parses");
    assert_eq!(got, StreamingListing::default());
}

#[test]
fn parse_streaming_episodes_response_returns_empty_when_field_is_null() {
    // Media present but streamingEpisodes itself is null — happens
    // for shows AniList catalogues without any Crunchyroll listing.
    let body = br#"{"data":{"Media":{"streamingEpisodes":null}}}"#;
    let got = parse_streaming_episodes_response(body).expect("parses");
    assert_eq!(got, StreamingListing::default());
}

#[test]
fn parse_streaming_episodes_response_rejects_non_envelope_payload() {
    let body = br#"<html>error</html>"#;
    let err = parse_streaming_episodes_response(body).unwrap_err();
    assert!(matches!(err, AniError::ParseFailed { .. }));
}

#[test]
fn parse_streaming_episodes_response_reads_the_count_and_status() {
    let body = br#"{"data":{"Media":{"episodes":12,"status":"FINISHED","streamingEpisodes":[]}}}"#;
    let got = parse_streaming_episodes_response(body).expect("parses");
    assert_eq!(got.episodes, Some(12));
    assert!(got.finished);
    let body =
        br#"{"data":{"Media":{"episodes":null,"status":"RELEASING","streamingEpisodes":[]}}}"#;
    let got = parse_streaming_episodes_response(body).expect("parses");
    assert_eq!(got.episodes, None);
    assert!(!got.finished);
}

#[test]
fn parse_streaming_episodes_response_keeps_the_first_entry_per_episode() {
    // Live AniList sometimes lists multiple streamingEpisodes for the
    // same ep number when a show has subbed + dubbed tracks both
    // registered with Crunchyroll; the first one is kept.
    let body = br#"{"data":{"Media":{"streamingEpisodes":[
        {"title":"Episode 1 - sub","thumbnail":"https://x.cdn/1-sub.jpg"},
        {"title":"Episode 1 - dub","thumbnail":"https://x.cdn/1-dub.jpg"},
        {"title":"Episode 2 - sub","thumbnail":"https://x.cdn/2.jpg"}
    ]}}}"#;
    let got = parse_streaming_episodes_response(body).expect("parses");
    assert_eq!(got.entries.len(), 2);
    assert_eq!(got.entries[&1].title, "sub");
    assert!(got.entries[&1]
        .thumbnail
        .as_deref()
        .is_some_and(|t| t.contains("1-sub")));
    assert!(got.entries[&2]
        .thumbnail
        .as_deref()
        .is_some_and(|t| t.contains("2.jpg")));
}

#[test]
fn split_episode_title_reads_the_number_and_the_words_after_it() {
    assert_eq!(
        parse::split_episode_title("Episode 7 - Seven"),
        Some((7, "Seven"))
    );
    assert_eq!(
        parse::split_episode_title("  episode 12: Twelve  "),
        Some((12, "Twelve"))
    );
    assert_eq!(parse::split_episode_title("Episode 3"), Some((3, "")));
    assert_eq!(parse::split_episode_title("Episode 3.5 - Recap"), None);
    assert_eq!(parse::split_episode_title("Episode - None"), None);
    assert_eq!(parse::split_episode_title("OVA 1 - Special"), None);
}

#[tokio::test]
async fn streaming_listing_for_media_by_mal_id_posts_correct_query() {
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
    let got = streaming_listing_for_media(&client, MediaRef::Mal(918), Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(
        got.entries.get(&1).and_then(|e| e.thumbnail.as_deref()),
        Some("https://x.cdn/1.jpg")
    );
}

#[tokio::test]
async fn streaming_listing_for_media_propagates_upstream_5xx() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(502))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let err = streaming_listing_for_media(&client, MediaRef::Mal(918), Some(&server.uri()))
        .await
        .unwrap_err();
    assert!(matches!(err, AniError::Upstream { status: 502 }));
}

#[tokio::test]
async fn streaming_listing_for_media_returns_empty_when_media_unmapped() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(404)
                .set_body_string(crate::meta::anilist_media::ANILIST_NOT_FOUND_BODY),
        )
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = streaming_listing_for_media(&client, MediaRef::Mal(99_999_999), Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got, StreamingListing::default());
}

#[tokio::test]
async fn streaming_listing_for_media_by_anilist_id_queries_media_by_id() {
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
    let got = streaming_listing_for_media(&client, MediaRef::AniList(207_141), Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(
        got.entries.get(&2).and_then(|e| e.thumbnail.as_deref()),
        Some("https://x.cdn/2.jpg")
    );
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
async fn streaming_listing_for_ids_retries_by_anilist_id_when_anilist_lacks_the_mal_id() {
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
    let got = streaming_listing_for_ids(&client, Some(21), Some(30), Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(
        got.entries.get(&3).and_then(|e| e.thumbnail.as_deref()),
        Some("https://x.cdn/3.jpg")
    );
}

#[tokio::test]
async fn streaming_listing_for_ids_does_not_retry_a_media_without_episodes() {
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
    let got = streaming_listing_for_ids(&client, Some(21), Some(30), Some(&server.uri()))
        .await
        .expect("ok");
    assert!(got.entries.is_empty());
}

proptest::proptest! {
    /// Every episode number AniList listed keeps the entry it was
    /// listed with first, and nothing else appears. Episode numbers
    /// come from a small range so duplicates are the common case.
    #[test]
    fn parse_keeps_each_episodes_first_entry(
        pairs in proptest::collection::vec((0u32..12, "[a-z]{1,6}"), 0..40),
    ) {
        let eps: Vec<serde_json::Value> = pairs
            .iter()
            .map(|(n, url)| serde_json::json!({
                "title": format!("Episode {n} - {url}"),
                "thumbnail": url,
            }))
            .collect();
        let body = serde_json::json!({ "data": { "Media": { "streamingEpisodes": eps } } });
        let got = parse_streaming_episodes_response(body.to_string().as_bytes())
            .expect("parses");
        let mut want = BTreeMap::new();
        for (n, url) in &pairs {
            want.entry(*n).or_insert(StreamingEntry {
                title: url.clone(),
                thumbnail: Some(url.clone()),
            });
        }
        proptest::prop_assert_eq!(got.entries, want);
    }
}

#[test]
fn parse_keeps_a_numbered_entry_without_a_thumbnail() {
    // A row with no still still says the listing numbers that episode,
    // which the fit rule reads; it fills nothing.
    let body = br#"{"data":{"Media":{"episodes":2,"status":"FINISHED","streamingEpisodes":[
        {"title":"Episode 1 - One","thumbnail":"https://x.cdn/1.jpg"},
        {"title":"Episode 2 - Two","thumbnail":null},
        {"title":"Episode 3 - Three","thumbnail":null}
    ]}}}"#;
    let got = parse_streaming_episodes_response(body).expect("parses");
    let nums: Vec<u32> = got.entries.keys().copied().collect();
    assert_eq!(nums, vec![1, 2, 3]);
    assert_eq!(got.entries[&3].title, "Three");
}
