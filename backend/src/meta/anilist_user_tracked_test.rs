//! AniList's `tracked_media`: the viewer's row and the media's episode
//! total from the one `Media` query the write-back already makes.

use super::*;
use wiremock::matchers::{body_string_contains, method};

async fn tracked(body: &'static str) -> TrackedMedia {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .and(body_string_contains("episodes"))
        .and(body_string_contains("mediaListEntry"))
        .and(body_string_contains("\"mediaId\":21"))
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
        .tracked_media(&tokens, ProviderMediaId(21))
        .await
        .expect("tracked_media ok")
}

#[tokio::test]
async fn tracked_media_reads_the_row_and_the_episode_total_in_one_request() {
    let got = tracked(
        r#"{"data":{"Media":{"episodes":13,"mediaListEntry":{"status":"CURRENT","progress":11}}}}"#,
    )
    .await;
    assert_eq!(
        got,
        TrackedMedia {
            entry: Some(CurrentEntry {
                status: crate::account::status::ListStatus::Watching,
                progress_episodes: 11,
                times_rewatched: None,
            }),
            episodes: Some(13),
        }
    );
}

#[tokio::test]
async fn tracked_media_knows_the_total_of_a_show_not_on_the_list() {
    let got = tracked(r#"{"data":{"Media":{"episodes":13,"mediaListEntry":null}}}"#).await;
    assert_eq!(
        got,
        TrackedMedia {
            entry: None,
            episodes: Some(13),
        }
    );
}

#[tokio::test]
async fn tracked_media_reads_a_null_total_as_unknown() {
    let got = tracked(
        r#"{"data":{"Media":{"episodes":null,"mediaListEntry":{"status":"CURRENT","progress":3}}}}"#,
    )
    .await;
    assert_eq!(got.episodes, None);
}
