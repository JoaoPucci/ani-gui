//! A tracker answering a request with a 4xx has answered: it rejected
//! the request (or, for 404/410, does not have what was asked for). That
//! is not the tracker failing, so the route layer must not answer it as
//! a 502 the renderer reads as a server fault and retries. Every 4xx a
//! tracker's authenticated API or token endpoint sends that is not a
//! refused bearer (401), a rate limit (429) or the tracker not taking
//! the request this time (408 timeout, 425 too early) reaches the route
//! as the tracker's rejection, `error.account.tracker_rejected`, HTTP
//! 422, carrying the tracker's status; a 5xx, a 408 and a 425 stay the
//! upstream failing, 502, which the renderer asks again.

use crate::account::provider::{EntryUpdate, ProviderMediaId, Tokens, UserListProvider};
use crate::error::AniError;
use crate::meta::anilist_user::AniListProvider;
use crate::meta::mal_user::{MalProvider, MalRefreshState};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn tokens() -> Tokens {
    Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

async fn answering(status: u16) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(status))
        .mount(&server)
        .await;
    server
}

fn mal(server: &MockServer) -> MalProvider {
    MalProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        server.uri(),
        MalRefreshState::new(),
    )
}

fn anilist(server: &MockServer) -> AniListProvider {
    AniListProvider::with_bases(reqwest::Client::new(), server.uri(), server.uri())
}

fn assert_rejected(err: &AniError, status: u16) {
    assert_eq!(err.key(), "error.account.tracker_rejected", "{err:?}");
    assert_eq!(err.http_status_code(), 422, "{err:?}");
    assert_eq!(
        serde_json::to_value(err).expect("envelope")["status"],
        status,
        "{err:?}"
    );
}

fn watched() -> EntryUpdate {
    EntryUpdate {
        progress_episodes: Some(1),
        ..Default::default()
    }
}

#[tokio::test]
async fn mal_rejecting_a_read_is_the_tracker_s_rejection() {
    for status in [400, 403, 404, 410, 418] {
        let server = answering(status).await;
        let err = mal(&server).me(&tokens()).await.expect_err("rejected");
        assert_rejected(&err, status);
    }
}

#[tokio::test]
async fn mal_rejecting_a_write_is_the_tracker_s_rejection() {
    let server = answering(403).await;
    let err = mal(&server)
        .update_entry(&tokens(), ProviderMediaId(21), watched())
        .await
        .expect_err("rejected");
    assert_rejected(&err, 403);
}

#[tokio::test]
async fn mal_failing_stays_the_upstream_failing() {
    let server = answering(503).await;
    let err = mal(&server).me(&tokens()).await.expect_err("failed");
    assert!(matches!(err, AniError::Upstream { status: 503 }), "{err:?}");
    assert_eq!(err.http_status_code(), 502);
}

#[tokio::test]
async fn anilist_rejecting_a_query_is_the_tracker_s_rejection() {
    for status in [400, 403, 404] {
        let server = answering(status).await;
        let err = anilist(&server).me(&tokens()).await.expect_err("rejected");
        assert_rejected(&err, status);
    }
}

#[tokio::test]
async fn anilist_failing_stays_the_upstream_failing() {
    let server = answering(500).await;
    let err = anilist(&server)
        .current_entry(&tokens(), ProviderMediaId(21))
        .await
        .expect_err("failed");
    assert!(matches!(err, AniError::Upstream { status: 500 }), "{err:?}");
    assert_eq!(err.http_status_code(), 502);
}

#[tokio::test]
async fn a_rejected_media_read_is_still_absence() {
    // The tracker's 404 for the entry is still read as the entry not
    // being there, wherever a caller asks.
    let server = answering(404).await;
    let err = mal(&server)
        .current_entry(&tokens(), ProviderMediaId(21))
        .await
        .expect_err("absent");
    assert!(err.is_not_found_shaped());
    assert!(matches!(
        AniError::unless_absent(Err::<(), _>(err)),
        Ok(None)
    ));
}

#[tokio::test]
async fn a_rejected_refresh_token_is_still_a_refused_sign_in() {
    for status in [400, 401] {
        let server = answering(status).await;
        let err = mal(&server).refresh("dead").await.expect_err("refused");
        assert!(
            matches!(err, AniError::RefreshRejected),
            "{status}: {err:?}"
        );
    }
}

/// The answer the route sends for `err`: its HTTP status and envelope
/// kind, as the renderer receives them.
fn answered(err: AniError) -> (u16, String) {
    use axum::response::IntoResponse;
    let kind = serde_json::to_value(&err).expect("envelope")["kind"]
        .as_str()
        .expect("kind")
        .to_owned();
    (err.into_response().status().as_u16(), kind)
}

/// The statuses a tracker sends when it did not take the request this
/// time: asking again can be answered.
const TRANSIENT: [u16; 6] = [408, 425, 500, 502, 503, 504];

#[tokio::test]
async fn a_tracker_that_did_not_answer_this_time_is_answered_as_retryable() {
    for status in TRANSIENT {
        let server = answering(status).await;
        let mal_err = mal(&server)
            .current_entry(&tokens(), ProviderMediaId(21))
            .await
            .expect_err("failed");
        assert_eq!(
            answered(mal_err),
            (502, "upstream".to_owned()),
            "MAL {status}"
        );
        let anilist_err = anilist(&server)
            .current_entry(&tokens(), ProviderMediaId(21))
            .await
            .expect_err("failed");
        assert_eq!(
            answered(anilist_err),
            (502, "upstream".to_owned()),
            "AniList {status}"
        );
    }
}

#[tokio::test]
async fn a_tracker_that_did_not_answer_a_write_this_time_is_answered_as_retryable() {
    for status in TRANSIENT {
        let server = answering(status).await;
        let err = mal(&server)
            .update_entry(&tokens(), ProviderMediaId(21), watched())
            .await
            .expect_err("failed");
        assert_eq!(answered(err), (502, "upstream".to_owned()), "MAL {status}");
    }
}

#[tokio::test]
async fn a_rate_limited_live_read_is_answered_as_a_rate_limit() {
    let server = answering(429).await;
    let err = mal(&server)
        .current_entry(&tokens(), ProviderMediaId(21))
        .await
        .expect_err("limited");
    assert_eq!(answered(err), (429, "rate_limited".to_owned()));
}

#[tokio::test]
async fn a_tracker_rejecting_a_live_read_is_answered_as_permanent() {
    for status in [400, 403, 418, 422] {
        let server = answering(status).await;
        let err = mal(&server)
            .current_entry(&tokens(), ProviderMediaId(21))
            .await
            .expect_err("rejected");
        assert_eq!(
            answered(err),
            (422, "tracker_rejected".to_owned()),
            "{status}"
        );
    }
}
