//! What a refused MAL refresh grant tells the caller.
//!
//! The renderer has to tell two things apart when renewing a sign-in:
//! MAL refusing the refresh token, after which only a new sign-in
//! helps, and MAL not answering, after which the same sign-in may
//! still work. A 400 or 401 from the token endpoint is the first: the
//! request is refused, and sending it again will be refused again. A
//! 5xx, a 429 or a transport failure is the second. Every other
//! status says neither and keeps the endpoint's own error.

use super::*;
use crate::error::AniError;
use crate::meta::mal_user_net::refresh_refusal;

const REFRESH_REJECTED: &str = "error.account.refresh_rejected";

fn make_provider(token_uri: &str) -> MalProvider {
    MalProvider::with_bases(
        reqwest::Client::new(),
        "http://unused-api".to_string(),
        token_uri.to_string(),
        MalRefreshState::new(),
    )
}

proptest::proptest! {
    /// A 400 or 401 is a refusal of the token; any other status keeps
    /// the endpoint's error exactly, so a 5xx still reads as the
    /// service failing and nothing else is renamed.
    #[test]
    fn only_a_400_or_401_reads_as_a_refused_token(status in 100u16..600) {
        let out = refresh_refusal(AniError::Upstream { status });
        if status == 400 || status == 401 {
            proptest::prop_assert_eq!(out.key(), REFRESH_REJECTED);
            proptest::prop_assert_eq!(out.http_status_code(), 401);
        } else {
            let kept = matches!(out, AniError::Upstream { status: s } if s == status);
            proptest::prop_assert!(kept, "status {} became {:?}", status, out);
        }
    }

    /// A rate limit, whatever wait it advertises, stays a rate limit.
    #[test]
    fn a_rate_limit_is_not_a_refusal(wait in proptest::option::of(0u64..100_000)) {
        let out = refresh_refusal(AniError::RateLimited { retry_after_secs: wait, refused_by: None });
        let kept = matches!(out, AniError::RateLimited { retry_after_secs, .. } if retry_after_secs == wait);
        proptest::prop_assert!(kept, "{:?}", out);
    }
}

#[test]
fn a_failure_to_reach_the_endpoint_is_not_a_refusal() {
    for e in [AniError::Network, AniError::Timeout] {
        let key = e.key();
        assert_eq!(refresh_refusal(e).key(), key);
    }
}

async fn refresh_answered(status: u16, body: &str) -> AniError {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(status).set_body_string(body))
        .mount(&server)
        .await;
    make_provider(&server.uri())
        .refresh("refresh-token")
        .await
        .expect_err("a refused grant is an error")
}

#[tokio::test]
async fn an_expired_refresh_token_is_a_refused_token() {
    let err = refresh_answered(
        400,
        r#"{"error":"invalid_grant","message":"The refresh token is invalid.","hint":"Token has expired"}"#,
    )
    .await;
    assert_eq!(err.key(), REFRESH_REJECTED);
}

#[tokio::test]
async fn a_revoked_refresh_token_is_a_refused_token() {
    let err = refresh_answered(
        401,
        r#"{"error":"invalid_request","message":"The refresh token is invalid."}"#,
    )
    .await;
    assert_eq!(err.key(), REFRESH_REJECTED);
}

#[tokio::test]
async fn a_failing_token_endpoint_is_not_a_refused_token() {
    let err = refresh_answered(503, "").await;
    assert!(
        matches!(err, AniError::Upstream { status: 503 }),
        "got {err:?}"
    );
}

#[tokio::test]
async fn a_refused_authorization_code_stays_the_endpoints_error() {
    // Only the refresh grant reads a 400 as a dead sign-in: a refused
    // code at connect time is the connect flow's own failure.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(400))
        .mount(&server)
        .await;
    let err = make_provider(&server.uri())
        .exchange_code("code", &crate::account::pkce::Pkce::new_plain())
        .await
        .expect_err("a refused code is an error");
    assert!(
        matches!(err, AniError::Upstream { status: 400 }),
        "got {err:?}"
    );
}
