//! Tests for `crate::meta::anilist_media`.

use super::*;
use crate::meta::anilist::BANNER_BY_MAL_GQL;

// --- MediaRef: which id an AniList query is keyed by ---------------

#[test]
fn media_ref_uses_the_mal_id_when_only_mal_is_known() {
    assert_eq!(
        MediaRef::preferring_mal(Some(21), None),
        Some(MediaRef::Mal(21))
    );
}

#[test]
fn media_ref_uses_the_anilist_id_when_only_anilist_is_known() {
    assert_eq!(
        MediaRef::preferring_mal(None, Some(207_141)),
        Some(MediaRef::AniList(207_141))
    );
}

#[test]
fn media_ref_keeps_the_mal_id_when_both_are_known() {
    assert_eq!(
        MediaRef::preferring_mal(Some(21), Some(30)),
        Some(MediaRef::Mal(21))
    );
}

#[test]
fn media_ref_is_none_when_neither_is_known() {
    assert_eq!(MediaRef::preferring_mal(None, None), None);
}

proptest::proptest! {
    /// A known id is never dropped, and a MAL id always wins.
    #[test]
    fn media_ref_never_drops_a_known_id(
        mal in proptest::option::of(proptest::num::u32::ANY),
        anilist in proptest::option::of(proptest::num::u32::ANY),
    ) {
        let got = MediaRef::preferring_mal(mal, anilist);
        proptest::prop_assert_eq!(got.is_some(), mal.is_some() || anilist.is_some());
        if let Some(m) = mal {
            proptest::prop_assert_eq!(got, Some(MediaRef::Mal(m)));
        }
    }
}

#[tokio::test]
async fn banner_for_media_by_anilist_id_queries_media_by_id() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("Media(id: $id"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "variables": { "id": 207_141 },
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Media":{"bannerImage":"https://example.com/yani.jpg"}}}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let got = banner_for_media(&client, MediaRef::AniList(207_141), Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.as_deref(), Some("https://example.com/yani.jpg"));
}

#[tokio::test]
async fn banner_for_media_by_mal_id_keeps_the_idmal_query() {
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
    let got = banner_for_media(&client, MediaRef::Mal(21), Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.as_deref(), Some("https://example.com/op.jpg"));
}
