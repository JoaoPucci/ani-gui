//! Tests for `crate::meta::anilist_media`.

use super::*;
use crate::meta::anilist::BANNER_BY_MAL_GQL;
use proptest::strategy::Strategy as _;

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

// --- One retry by AniList id after the MAL id finds no media -------

#[test]
fn a_mal_id_anilist_lacks_retries_by_the_known_anilist_id() {
    assert_eq!(
        MediaRef::Mal(21).retry_after_missing(Some(30)),
        Some(MediaRef::AniList(30))
    );
}

#[test]
fn a_mal_id_anilist_lacks_has_no_retry_without_an_anilist_id() {
    assert_eq!(MediaRef::Mal(21).retry_after_missing(None), None);
}

#[test]
fn an_anilist_id_lookup_has_no_retry() {
    assert_eq!(MediaRef::AniList(30).retry_after_missing(Some(30)), None);
}

proptest::proptest! {
    /// The retry is only ever the AniList id, and only after a MAL id.
    #[test]
    fn the_retry_is_the_anilist_id_after_a_mal_id(
        first_is_mal in proptest::bool::ANY,
        id in proptest::num::u32::ANY,
        anilist in proptest::option::of(proptest::num::u32::ANY),
    ) {
        let first = if first_is_mal { MediaRef::Mal(id) } else { MediaRef::AniList(id) };
        let want = if first_is_mal { anilist.map(MediaRef::AniList) } else { None };
        proptest::prop_assert_eq!(first.retry_after_missing(anilist), want);
    }
}

#[test]
fn media_is_absent_only_for_a_null_media() {
    assert!(media_is_absent(br#"{"data":{"Media":null}}"#));
    assert!(!media_is_absent(
        br#"{"data":{"Media":{"bannerImage":null}}}"#
    ));
    assert!(!media_is_absent(b"not json"));
}

/// Any JSON value, `null` included, nested a few levels deep.
fn arb_json() -> impl proptest::strategy::Strategy<Value = serde_json::Value> {
    use proptest::prelude::*;
    let leaf = prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::from),
        any::<i64>().prop_map(serde_json::Value::from),
        ".{0,8}".prop_map(serde_json::Value::from),
    ];
    leaf.prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::from),
            proptest::collection::btree_map("[a-z]{1,6}", inner, 0..4)
                .prop_map(|m| serde_json::Value::Object(m.into_iter().collect())),
        ]
    })
}

/// Any JSON value but `null`: what AniList puts at `data.Media` for a
/// media it does index.
fn arb_present_json() -> impl proptest::strategy::Strategy<Value = serde_json::Value> {
    use proptest::prelude::*;
    arb_json().prop_filter("not null", |v| !v.is_null())
}

/// Sibling keys an envelope may carry beside `Media` / `data`, none of
/// them the key the classifier reads.
fn arb_siblings() -> impl proptest::strategy::Strategy<Value = Vec<(String, serde_json::Value)>> {
    proptest::collection::vec(
        (
            "[a-zA-Z]{1,6}".prop_filter("not a key the classifier reads", |k| {
                k != "data" && k != "Media"
            }),
            arb_json(),
        ),
        0..3,
    )
}

fn envelope(
    media: Option<serde_json::Value>,
    data_siblings: &[(String, serde_json::Value)],
    top_siblings: &[(String, serde_json::Value)],
    errors: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut data = serde_json::Map::new();
    for (k, v) in data_siblings {
        data.insert(k.clone(), v.clone());
    }
    if let Some(m) = media {
        data.insert("Media".into(), m);
    }
    let mut top = serde_json::Map::new();
    for (k, v) in top_siblings {
        top.insert(k.clone(), v.clone());
    }
    if let Some(e) = errors {
        top.insert("errors".into(), e);
    }
    top.insert("data".into(), serde_json::Value::Object(data));
    serde_json::Value::Object(top)
}

proptest::proptest! {
    /// `data.Media: null` is absence whatever else the envelope
    /// carries — AniList's 404 puts an `errors` array beside it.
    #[test]
    fn a_null_media_is_absent_whatever_surrounds_it(
        data_siblings in arb_siblings(),
        top_siblings in arb_siblings(),
        errors in proptest::option::of(arb_json()),
    ) {
        let body = envelope(Some(serde_json::Value::Null), &data_siblings, &top_siblings, errors);
        proptest::prop_assert!(media_is_absent(&serde_json::to_vec(&body).unwrap()));
    }

    /// A media AniList returned, whatever its shape, is never absent —
    /// nor is an envelope with no `Media` key at all.
    #[test]
    fn a_present_or_missing_media_is_not_absent(
        media in proptest::option::of(arb_present_json()),
        data_siblings in arb_siblings(),
        top_siblings in arb_siblings(),
        errors in proptest::option::of(arb_json()),
    ) {
        let body = envelope(media, &data_siblings, &top_siblings, errors);
        proptest::prop_assert!(!media_is_absent(&serde_json::to_vec(&body).unwrap()));
    }

    /// `data` that is not an object holding `Media` says nothing about
    /// a media, `null` included.
    #[test]
    fn a_data_that_is_not_an_object_is_not_absent(
        data in arb_json().prop_filter("not an object", |v| !v.is_object()),
        errors in proptest::option::of(arb_json()),
    ) {
        let mut top = serde_json::Map::new();
        top.insert("data".into(), data);
        if let Some(e) = errors {
            top.insert("errors".into(), e);
        }
        let body = serde_json::to_vec(&serde_json::Value::Object(top)).unwrap();
        proptest::prop_assert!(!media_is_absent(&body));
    }

    /// A body cut short is unreadable, not absent, even when the whole
    /// of it would have said `Media: null`.
    #[test]
    fn a_truncated_absent_body_is_not_absent(
        errors in proptest::option::of(arb_json()),
        cut in proptest::num::usize::ANY,
    ) {
        let body = serde_json::to_vec(&envelope(Some(serde_json::Value::Null), &[], &[], errors)).unwrap();
        let cut = cut % body.len();
        proptest::prop_assert!(!media_is_absent(&body[..cut]));
    }
}

/// AniList mock answering `body` to queries whose variables match
/// `variables`, expecting exactly `times` of them.
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

const MEDIA_NULL: &str = crate::meta::anilist_media::ANILIST_NOT_FOUND_BODY;

#[tokio::test]
async fn banner_for_ids_retries_by_anilist_id_when_anilist_lacks_the_mal_id() {
    let server = wiremock::MockServer::start().await;
    mount_media(&server, serde_json::json!({ "idMal": 21 }), MEDIA_NULL, 1).await;
    mount_media(
        &server,
        serde_json::json!({ "id": 30 }),
        r#"{"data":{"Media":{"bannerImage":"https://al/30.jpg"}}}"#,
        1,
    )
    .await;
    let client = reqwest::Client::new();
    let got = banner_for_ids(&client, Some(21), Some(30), Some(&server.uri()))
        .await
        .expect("ok");
    assert_eq!(got.as_deref(), Some("https://al/30.jpg"));
}

#[tokio::test]
async fn banner_for_ids_does_not_retry_a_media_that_exists_without_a_banner() {
    let server = wiremock::MockServer::start().await;
    mount_media(
        &server,
        serde_json::json!({ "idMal": 21 }),
        r#"{"data":{"Media":{"bannerImage":null}}}"#,
        1,
    )
    .await;
    mount_media(&server, serde_json::json!({ "id": 30 }), MEDIA_NULL, 0).await;
    let client = reqwest::Client::new();
    let got = banner_for_ids(&client, Some(21), Some(30), Some(&server.uri()))
        .await
        .expect("ok");
    assert!(got.is_none());
}

#[tokio::test]
async fn banner_for_ids_stops_after_both_ids_find_no_media() {
    let server = wiremock::MockServer::start().await;
    mount_media(&server, serde_json::json!({ "idMal": 21 }), MEDIA_NULL, 1).await;
    mount_media(&server, serde_json::json!({ "id": 30 }), MEDIA_NULL, 1).await;
    let client = reqwest::Client::new();
    let got = banner_for_ids(&client, Some(21), Some(30), Some(&server.uri()))
        .await
        .expect("ok");
    assert!(got.is_none());
}
