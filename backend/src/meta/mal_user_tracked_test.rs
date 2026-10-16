//! MAL's `tracked_media`: the user's row and the anime's episode total
//! from the one `/anime/{id}` read the write-back already makes.

use super::*;
use wiremock::matchers::{method, path, query_param};

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

async fn tracked(body: &'static str) -> TrackedMedia {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/21"))
        .and(query_param("fields", "my_list_status,num_episodes"))
        .and(query_param("nsfw", "true"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(&server)
        .await;
    provider(&server.uri())
        .tracked_media(&tokens(), ProviderMediaId(21))
        .await
        .expect("tracked_media ok")
}

#[tokio::test]
async fn tracked_media_reads_the_row_and_the_episode_total_in_one_request() {
    let got = tracked(
        r#"{"id":21,"num_episodes":13,"my_list_status":{"status":"watching","num_episodes_watched":11}}"#,
    )
    .await;
    assert_eq!(
        got,
        TrackedMedia {
            entry: Some(CurrentEntry {
                status: crate::account::status::ListStatus::Watching,
                progress_episodes: 11,
            }),
            episodes: Some(13),
        }
    );
}

#[tokio::test]
async fn tracked_media_knows_the_total_of_a_show_not_on_the_list() {
    let got = tracked(r#"{"id":21,"num_episodes":13}"#).await;
    assert_eq!(
        got,
        TrackedMedia {
            entry: None,
            episodes: Some(13),
        }
    );
}

#[tokio::test]
async fn tracked_media_reads_a_zero_total_as_unknown() {
    // MAL answers 0 for a show whose length isn't announced.
    let got = tracked(r#"{"id":21,"num_episodes":0}"#).await;
    assert_eq!(got.episodes, None);
}
