//! Identical public queries sent at the same time share one request.

use wiremock::matchers::{body_string_contains, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{media_id_for_mal, request_key};
use crate::meta::anilist_gate::{in_background, Class};

const MEDIA: &str = r#"{"data":{"Media":{"id":700}}}"#;

/// AniList answering the MAL-id lookup after a moment, expecting
/// `times` requests in all.
async fn anilist(times: u64) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains("MediaIdByMal"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(MEDIA)
                .set_delay(std::time::Duration::from_millis(100)),
        )
        .expect(times)
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn the_same_query_asked_twice_at_once_is_one_request() {
    let server = anilist(1).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let (a, b) = tokio::join!(
        media_id_for_mal(&client, 70, Some(&base)),
        media_id_for_mal(&client, 70, Some(&base)),
    );
    assert_eq!(a.expect("answered"), Some(700));
    assert_eq!(b.expect("answered"), Some(700));
}

#[tokio::test]
async fn different_queries_at_once_each_go_out() {
    let server = anilist(2).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let (a, b) = tokio::join!(
        media_id_for_mal(&client, 70, Some(&base)),
        media_id_for_mal(&client, 71, Some(&base)),
    );
    a.expect("answered");
    b.expect("answered");
}

#[tokio::test]
async fn a_background_fill_and_a_read_the_user_waits_on_do_not_share() {
    // A fill the gate may hold back must not hand its refusal to a read
    // the user is waiting on, so the two go out on their own.
    let server = anilist(2).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let (a, b) = tokio::join!(
        in_background(media_id_for_mal(&client, 70, Some(&base))),
        media_id_for_mal(&client, 70, Some(&base)),
    );
    a.expect("answered");
    b.expect("answered");
}

#[tokio::test]
async fn the_same_query_asked_after_the_first_answered_goes_out_again() {
    let server = anilist(2).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    media_id_for_mal(&client, 70, Some(&base))
        .await
        .expect("first");
    media_id_for_mal(&client, 70, Some(&base))
        .await
        .expect("second");
}

proptest::proptest! {
    /// Two requests share a key exactly when class, endpoint and body all
    /// agree.
    #[test]
    fn requests_share_a_key_exactly_when_class_endpoint_and_body_agree(
        background in proptest::bool::ANY,
        other_class in proptest::bool::ANY,
        url in "https://[a-z]{1,8}\\.co/[a-z]{0,4}",
        other_url in "https://[a-z]{1,8}\\.co/[a-z]{0,4}",
        id in 0u32..50,
        other_id in 0u32..50,
    ) {
        let class = |b: bool| if b { Class::Background } else { Class::Interactive };
        let body = |id: u32| serde_json::json!({ "query": "q", "variables": { "idMal": id } });
        let same = background == other_class && url == other_url && id == other_id;
        proptest::prop_assert_eq!(
            request_key(class(background), &url, &body(id))
                == request_key(class(other_class), &other_url, &body(other_id)),
            same
        );
    }
}
