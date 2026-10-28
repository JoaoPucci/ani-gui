//! A bearer's user is asked of the tracker once.

use wiremock::matchers::{body_string_contains, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::OwnerMemo;
use crate::account::provider::{ProviderKind, Tokens};
use crate::meta::anilist_user::AniListProvider;

const VIEWER: &str = r#"{"data":{"Viewer":{"id":4242,"name":"shiro","avatar":{"large":null,"medium":null},"statistics":{"anime":{"count":0,"meanScore":0}}}}}"#;

fn bearer(token: &str) -> Tokens {
    Tokens {
        access_token: token.to_owned(),
        refresh_token: None,
        expires_at_epoch_s: 0,
    }
}

async fn anilist_answering(times: u64) -> (MockServer, AniListProvider) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains("Viewer"))
        .respond_with(ResponseTemplate::new(200).set_body_string(VIEWER))
        .expect(times)
        .mount(&server)
        .await;
    let provider = AniListProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_owned(),
    );
    (server, provider)
}

#[tokio::test]
async fn a_bearer_s_user_is_asked_once_and_answered_from_then_on() {
    let (_server, provider) = anilist_answering(1).await;
    let memo = OwnerMemo::default();
    for _ in 0..3 {
        let user = memo
            .owner(ProviderKind::AniList, &provider, &bearer("tok-a"))
            .await
            .expect("owner");
        assert_eq!(user, "4242");
    }
    // `.expect(1)`: one Viewer query for three asks.
}

#[tokio::test]
async fn each_bearer_is_asked_on_its_own() {
    let (_server, provider) = anilist_answering(2).await;
    let memo = OwnerMemo::default();
    for token in ["tok-a", "tok-b", "tok-a", "tok-b"] {
        memo.owner(ProviderKind::AniList, &provider, &bearer(token))
            .await
            .expect("owner");
    }
}

#[tokio::test]
async fn a_refused_lookup_is_kept_nowhere_and_the_next_ask_goes_out() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains("Viewer"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("Viewer"))
        .respond_with(ResponseTemplate::new(200).set_body_string(VIEWER))
        .mount(&server)
        .await;
    let provider = AniListProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_owned(),
    );
    let memo = OwnerMemo::default();
    assert!(memo
        .owner(ProviderKind::AniList, &provider, &bearer("tok-a"))
        .await
        .is_err());
    assert_eq!(
        memo.owner(ProviderKind::AniList, &provider, &bearer("tok-a"))
            .await
            .expect("owner"),
        "4242"
    );
}

#[tokio::test]
async fn a_disconnect_forgets_the_tracker_s_bearers() {
    let (_server, provider) = anilist_answering(2).await;
    let memo = OwnerMemo::default();
    memo.owner(ProviderKind::AniList, &provider, &bearer("tok-a"))
        .await
        .expect("owner");
    memo.forget(ProviderKind::MyAnimeList);
    memo.owner(ProviderKind::AniList, &provider, &bearer("tok-a"))
        .await
        .expect("owner (kept: another tracker was forgotten)");
    memo.forget(ProviderKind::AniList);
    memo.owner(ProviderKind::AniList, &provider, &bearer("tok-a"))
        .await
        .expect("owner (asked again)");
}
