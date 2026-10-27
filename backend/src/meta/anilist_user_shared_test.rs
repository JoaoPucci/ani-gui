//! The account's identical queries sent at once share one request; its
//! writes never do.

use wiremock::matchers::{body_string_contains, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::account::provider::{EntryUpdate, ProviderMediaId, Tokens, UserListProvider};
use crate::meta::anilist_user::AniListProvider;

fn bearer(token: &str) -> Tokens {
    Tokens {
        access_token: token.to_owned(),
        refresh_token: None,
        expires_at_epoch_s: 0,
    }
}

fn provider(server: &MockServer) -> AniListProvider {
    AniListProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_owned(),
    )
}

async fn entry_read(server: &MockServer, times: u64) {
    Mock::given(method("POST"))
        .and(body_string_contains("mediaListEntry"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"data":{"Media":{"mediaListEntry":{"status":"CURRENT","progress":3}}}}"#,
                )
                .set_delay(std::time::Duration::from_millis(100)),
        )
        .expect(times)
        .mount(server)
        .await;
}

#[tokio::test]
async fn the_same_entry_read_for_one_bearer_at_once_is_one_request() {
    let server = MockServer::start().await;
    entry_read(&server, 1).await;
    let anilist = provider(&server);
    let tokens = bearer("tok-a");
    let (a, b) = tokio::join!(
        anilist.current_entry(&tokens, ProviderMediaId(21)),
        anilist.current_entry(&tokens, ProviderMediaId(21)),
    );
    assert_eq!(a.expect("read"), b.expect("read"));
}

#[tokio::test]
async fn the_same_read_for_two_bearers_goes_out_for_each() {
    let server = MockServer::start().await;
    entry_read(&server, 2).await;
    let anilist = provider(&server);
    let (first, second) = (bearer("tok-a"), bearer("tok-b"));
    let (a, b) = tokio::join!(
        anilist.current_entry(&first, ProviderMediaId(21)),
        anilist.current_entry(&second, ProviderMediaId(21)),
    );
    a.expect("read");
    b.expect("read");
}

#[tokio::test]
async fn two_identical_writes_at_once_each_go_out() {
    // A write is never folded into another: each the user made is sent.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains("SaveMediaListEntry"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"data":{"SaveMediaListEntry":{"id":1,"mediaId":21,"status":"CURRENT","progress":3,"score":0,"updatedAt":1,"media":{"idMal":21,"title":{"romaji":"x","english":null}}}}}"#,
                )
                .set_delay(std::time::Duration::from_millis(100)),
        )
        .expect(2)
        .mount(&server)
        .await;
    let anilist = provider(&server);
    let tokens = bearer("tok-a");
    let update = || EntryUpdate {
        progress_episodes: Some(3),
        ..Default::default()
    };
    let (a, b) = tokio::join!(
        anilist.update_entry(&tokens, ProviderMediaId(21), update()),
        anilist.update_entry(&tokens, ProviderMediaId(21), update()),
    );
    let _ = (a, b);
}
