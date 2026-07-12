//! Tests for `crate::meta::anilist`. Extracted via `#[path]` so the
//! test-fn complexity does not count against `anilist.rs`'s CRAP
//! budget — per the same convention as `anilist_streaming_eps_test.rs`.

use super::*;

/// Real shape lifted from a live AniList trending probe. Three
/// entries; covers the optional fields (idMal, episodes,
/// bannerImage) hitting both Some and null variants so the
/// derive's `Option` handling is exercised.
fn fixture_body() -> &'static [u8] {
    // Regular raw string (not byte raw) because the JSON contains
    // Japanese characters; `br#"…"#` rejects non-ASCII.
    FIXTURE_JSON.as_bytes()
}

const FIXTURE_JSON: &str = r##"{
        "data": {
            "Page": {
                "media": [
                    {
                        "id": 182205,
                        "idMal": 59970,
                        "title": {
                            "romaji": "Tensei Shitara Slime Datta Ken 4th Season",
                            "english": "That Time I Got Reincarnated as a Slime Season 4",
                            "native": "転生したらスライムだった件 第4期",
                            "userPreferred": "Tensei Shitara Slime Datta Ken 4th Season"
                        },
                        "coverImage": {
                            "extraLarge": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/bx182205-q.jpg",
                            "large": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/medium/bx182205-q.jpg",
                            "medium": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/small/bx182205-q.jpg",
                            "color": "#1abbd6"
                        },
                        "bannerImage": "https://s4.anilist.co/file/anilistcdn/media/anime/banner/182205-f.jpg",
                        "status": "RELEASING",
                        "episodes": null,
                        "trending": 273,
                        "averageScore": 80
                    },
                    {
                        "id": 21,
                        "idMal": 21,
                        "title": {
                            "romaji": "ONE PIECE",
                            "english": "ONE PIECE",
                            "native": "ONE PIECE",
                            "userPreferred": "ONE PIECE"
                        },
                        "coverImage": {
                            "extraLarge": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/bx21-E.jpg",
                            "large": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/medium/bx21-E.jpg",
                            "medium": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/small/bx21-E.jpg",
                            "color": "#e49335"
                        },
                        "bannerImage": "https://s4.anilist.co/file/anilistcdn/media/anime/banner/21-w.jpg",
                        "status": "RELEASING",
                        "episodes": null,
                        "trending": 167,
                        "averageScore": 87
                    },
                    {
                        "id": 999999,
                        "idMal": null,
                        "title": {
                            "romaji": "Hypothetical Show With No MAL Id",
                            "english": null,
                            "native": null,
                            "userPreferred": "Hypothetical Show With No MAL Id"
                        },
                        "coverImage": {
                            "extraLarge": null,
                            "large": null,
                            "medium": null,
                            "color": null
                        },
                        "bannerImage": null,
                        "status": null,
                        "episodes": null,
                        "trending": null,
                        "averageScore": null
                    }
                ]
            }
        }
    }"##;

#[test]
fn parse_trending_yields_expected_count_and_first_entry() {
    let v = parse_trending(fixture_body()).expect("parses");
    assert_eq!(v.len(), 3);
    let first = &v[0];
    assert_eq!(first.id, 182205);
    assert_eq!(first.id_mal, Some(59970));
    assert_eq!(
        first.title.user_preferred.as_deref(),
        Some("Tensei Shitara Slime Datta Ken 4th Season")
    );
    assert_eq!(first.cover_image.color.as_deref(), Some("#1abbd6"));
    assert!(first.banner_image.is_some());
    assert_eq!(first.status.as_deref(), Some("RELEASING"));
    assert_eq!(first.trending, Some(273));
}

#[test]
fn parse_trending_handles_missing_optionals() {
    let v = parse_trending(fixture_body()).expect("parses");
    let third = &v[2];
    assert_eq!(third.id, 999999);
    assert_eq!(third.id_mal, None);
    assert_eq!(third.title.english, None);
    assert_eq!(third.cover_image.extra_large, None);
    assert_eq!(third.banner_image, None);
    assert_eq!(third.status, None);
    assert_eq!(third.episodes, None);
    assert_eq!(third.trending, None);
    assert_eq!(third.average_score, None);
}

#[test]
fn parse_trending_rejects_html_or_garbage() {
    let r = parse_trending(b"<html>not json</html>");
    assert!(matches!(r, Err(AniError::ParseFailed { .. })));
}

#[tokio::test]
async fn trending_makes_correct_post_request() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/"))
        .and(wiremock::matchers::header(
            "content-type",
            "application/json",
        ))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "query": TRENDING_GQL,
            "variables": { "perPage": 5 },
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_bytes(fixture_body()))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let v = trending(&client, 5, Some(&server.uri())).await.expect("ok");
    assert_eq!(v.len(), 3);
    assert_eq!(v[0].id, 182205);
}

#[tokio::test]
async fn trending_propagates_5xx_as_upstream() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let err = trending(&client, 5, Some(&server.uri())).await.unwrap_err();
    assert!(matches!(err, AniError::Upstream { status: 503 }));
}

#[tokio::test]
async fn trending_surfaces_429_for_rate_limit() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(429))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let err = trending(&client, 5, Some(&server.uri())).await.unwrap_err();
    assert!(matches!(err, AniError::Upstream { status: 429 }));
}

/// `parse_banner_response` is the pure half of the banner-
/// backfill flow. Three branches matter: media present + banner
/// present, media present + banner null, and media itself null
/// (AniList didn't index this MAL id at all).
#[test]
fn parse_banner_response_returns_url_when_present() {
    let body = br#"{"data":{"Media":{"bannerImage":"https://example.com/b.jpg"}}}"#;
    let got = parse_banner_response(body).expect("ok");
    assert_eq!(got.as_deref(), Some("https://example.com/b.jpg"));
}

#[test]
fn parse_banner_response_returns_none_when_banner_is_null() {
    let body = br#"{"data":{"Media":{"bannerImage":null}}}"#;
    let got = parse_banner_response(body).expect("ok");
    assert!(got.is_none());
}

#[test]
fn parse_banner_response_returns_none_when_media_is_null() {
    // AniList answers with `Media: null` when the MAL id isn't
    // in their catalogue. Treating that as a hard error would
    // break the detail page for any show AniList missed.
    let body = br#"{"data":{"Media":null}}"#;
    let got = parse_banner_response(body).expect("ok");
    assert!(got.is_none());
}

#[test]
fn parse_banner_response_rejects_non_envelope_payload() {
    let body = br#"<html>error</html>"#;
    let err = parse_banner_response(body).unwrap_err();
    assert!(matches!(err, AniError::ParseFailed { .. }));
}

#[tokio::test]
async fn banner_for_mal_id_posts_correct_query() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "query": BANNER_BY_MAL_GQL,
            "variables": { "idMal": 21 },
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"data":{"Media":{"bannerImage":"https://example.com/op.jpg"}}}"#,
            ),
        )
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = banner_for_mal_id(&client, 21, Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.as_deref(), Some("https://example.com/op.jpg"));
}

#[tokio::test]
async fn banner_for_mal_id_propagates_upstream_5xx() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(502))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let err = banner_for_mal_id(&client, 21, Some(&server.uri()))
        .await
        .unwrap_err();
    assert!(matches!(err, AniError::Upstream { status: 502 }));
}

#[tokio::test]
async fn banner_for_mal_id_returns_none_when_media_unmapped() {
    // End-to-end through the network layer for the "AniList
    // doesn't have this MAL id" case — the most common failure
    // mode in production for niche shows.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#),
        )
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = banner_for_mal_id(&client, 99_999_999, Some(&server.uri()))
        .await
        .expect("ok");
    assert!(got.is_none());
}

// `media_id_for_mal` resolves a MAL id → AniList numeric mediaId,
// the keystone the write-back path needs: mark-watched knows the
// Kitsu id → mal_id, but SaveMediaListEntry wants AniList's own id.
#[test]
fn parse_media_id_response_returns_id_when_present() {
    let body = br#"{"data":{"Media":{"id":154587}}}"#;
    let got = parse_media_id_response(body).expect("ok");
    assert_eq!(got, Some(154587));
}

#[test]
fn parse_media_id_response_returns_none_when_media_is_null() {
    // AniList answers Media: null when the MAL id isn't indexed.
    let body = br#"{"data":{"Media":null}}"#;
    let got = parse_media_id_response(body).expect("ok");
    assert!(got.is_none());
}

#[test]
fn parse_media_id_response_rejects_non_envelope_payload() {
    let err = parse_media_id_response(br#"<html>nope</html>"#).unwrap_err();
    assert!(matches!(err, AniError::ParseFailed { .. }));
}

#[tokio::test]
async fn media_id_for_mal_posts_idmal_query_and_returns_media_id() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "query": MEDIA_ID_BY_MAL_GQL,
            "variables": { "idMal": 52991 },
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"Media":{"id":154587}}}"#),
        )
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = media_id_for_mal(&client, 52991, Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got, Some(154587));
}

#[tokio::test]
async fn media_id_for_mal_returns_none_when_unmapped() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#),
        )
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = media_id_for_mal(&client, 99_999_999, Some(&server.uri()))
        .await
        .expect("ok");
    assert!(got.is_none());
}

#[tokio::test]
async fn media_id_for_mal_propagates_upstream_5xx() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(502))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let err = media_id_for_mal(&client, 52991, Some(&server.uri()))
        .await
        .unwrap_err();
    assert!(matches!(err, AniError::Upstream { status: 502 }));
}

// `mal_id_for_media_id` is the inverse bridge: Kitsu shows that
// carry only the anilist/anime mapping (Yani Neko, AniList 207141)
// still need a MAL id to write to MyAnimeList.
#[test]
fn parse_mal_id_response_returns_idmal_when_present() {
    let body = br#"{"data":{"Media":{"idMal":63403}}}"#;
    let got = parse_mal_id_response(body).expect("ok");
    assert_eq!(got, Some(63403));
}

#[test]
fn parse_mal_id_response_returns_none_when_idmal_is_null() {
    // AniList-only originals carry Media.idMal: null.
    let body = br#"{"data":{"Media":{"idMal":null}}}"#;
    let got = parse_mal_id_response(body).expect("ok");
    assert!(got.is_none());
}

#[test]
fn parse_mal_id_response_returns_none_when_media_is_null() {
    let body = br#"{"data":{"Media":null}}"#;
    let got = parse_mal_id_response(body).expect("ok");
    assert!(got.is_none());
}

#[test]
fn parse_mal_id_response_rejects_non_envelope_payload() {
    let err = parse_mal_id_response(br#"<html>nope</html>"#).unwrap_err();
    assert!(matches!(err, AniError::ParseFailed { .. }));
}

#[tokio::test]
async fn mal_id_for_media_id_posts_media_query_and_returns_idmal() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "query": MAL_ID_BY_MEDIA_GQL,
            "variables": { "id": 207141 },
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"Media":{"idMal":63403}}}"#),
        )
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = mal_id_for_media_id(&client, 207141, Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got, Some(63403));
}

#[tokio::test]
async fn mal_id_for_media_id_propagates_upstream_5xx() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(502))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let err = mal_id_for_media_id(&client, 207141, Some(&server.uri()))
        .await
        .unwrap_err();
    assert!(matches!(err, AniError::Upstream { status: 502 }));
}

// `media_ids_for_mals` is the batched inverse bridge: the Watch-Later
// rail resolves ALL its Kitsu-unmapped MAL ids in one Page query per
// 50-chunk instead of one Media query each (Codex P2 #3565216298).
#[test]
fn parse_media_ids_by_mal_response_builds_the_idmal_map() {
    let body = br#"{"data":{"Page":{"media":[
        {"id":207141,"idMal":63403},
        {"id":207142,"idMal":63404},
        {"id":300000,"idMal":null}
    ]}}}"#;
    let got = parse_media_ids_by_mal_response(body).expect("ok");
    assert_eq!(got.len(), 2);
    assert_eq!(got.get(&63403), Some(&207141));
    assert_eq!(got.get(&63404), Some(&207142));
}

#[test]
fn parse_media_ids_by_mal_response_empty_page_is_empty_map() {
    let body = br#"{"data":{"Page":{"media":[]}}}"#;
    let got = parse_media_ids_by_mal_response(body).expect("ok");
    assert!(got.is_empty());
}

#[test]
fn parse_media_ids_by_mal_response_rejects_non_envelope_payload() {
    let err = parse_media_ids_by_mal_response(br#"<html>nope</html>"#).unwrap_err();
    assert!(matches!(err, AniError::ParseFailed { .. }));
}

#[tokio::test]
async fn media_ids_for_mals_posts_one_query_for_a_small_batch() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "query": MEDIA_IDS_BY_MALS_GQL,
            "variables": { "idMals": [63403, 63404] },
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Page":{"media":[{"id":207141,"idMal":63403},{"id":207142,"idMal":63404}]}}}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = media_ids_for_mals(&client, &[63403, 63404], Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.get(&63403), Some(&207141));
    assert_eq!(got.get(&63404), Some(&207142));
}

#[tokio::test]
async fn media_ids_for_mals_makes_no_request_for_empty_input() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("{}"))
        .expect(0)
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = media_ids_for_mals(&client, &[], Some(&server.uri()))
        .await
        .expect("ok");
    assert!(got.is_empty());
}

#[tokio::test]
async fn media_ids_for_mals_chunks_batches_over_the_page_cap() {
    // 60 ids must split into two requests (50 + 10) — Page caps
    // perPage at 50 and would silently truncate a single oversized
    // batch.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"Page":{"media":[]}}}"#),
        )
        .expect(2)
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let ids: Vec<u32> = (1..=60).collect();
    let got = media_ids_for_mals(&client, &ids, Some(&server.uri()))
        .await
        .expect("ok");
    assert!(got.is_empty());
}
