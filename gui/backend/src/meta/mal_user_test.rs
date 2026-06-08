//! Tests for `crate::meta::mal_user::MalProvider`. Extracted via `#[path]`
//! so wiremock fixtures + helper structures don't pile onto the
//! production module's CCN budget — per `project_crap_inline_test_gotcha`.

use super::*;
use crate::account::credentials::{MAL_AUTH_URL, MAL_CLIENT_ID, MAL_REDIRECT_URI};
use crate::account::pkce::{Pkce, PkceMethod};

/// Build the wiremock-backed provider used across the network tests.
/// `api_uri` / `token_uri` are wiremock server URIs.
#[allow(dead_code)] // Used once network tests land — keep the helper compiled.
fn make_provider(api_uri: &str, token_uri: &str) -> MalProvider {
    MalProvider::with_bases(
        reqwest::Client::new(),
        api_uri.to_string(),
        token_uri.to_string(),
    )
}

fn production_provider() -> MalProvider {
    MalProvider::new(reqwest::Client::new())
}

fn plain_pkce() -> Pkce {
    Pkce::new_plain()
}

#[test]
fn auth_url_starts_with_the_mal_authorize_endpoint() {
    let url = production_provider().auth_url(&plain_pkce(), "csrf-token");
    assert!(
        url.starts_with(MAL_AUTH_URL),
        "auth_url must point at MAL's authorize endpoint, got: {url}"
    );
}

#[test]
fn auth_url_carries_the_public_client_id() {
    let url = production_provider().auth_url(&plain_pkce(), "csrf-token");
    assert!(
        url.contains(&format!("client_id={MAL_CLIENT_ID}")),
        "auth_url must carry the public client_id, got: {url}"
    );
}

#[test]
fn auth_url_round_trips_the_csrf_state() {
    let url = production_provider().auth_url(&plain_pkce(), "csrf-token-xyz");
    assert!(
        url.contains("state=csrf-token-xyz"),
        "auth_url must round-trip the CSRF state, got: {url}"
    );
}

#[test]
fn auth_url_uses_the_registered_redirect_uri() {
    let url = production_provider().auth_url(&plain_pkce(), "csrf");
    let encoded =
        url::form_urlencoded::byte_serialize(MAL_REDIRECT_URI.as_bytes()).collect::<String>();
    assert!(
        url.contains(&format!("redirect_uri={encoded}")),
        "auth_url must declare the registered redirect_uri, got: {url}"
    );
}

#[test]
fn auth_url_declares_response_type_code() {
    let url = production_provider().auth_url(&plain_pkce(), "csrf");
    assert!(
        url.contains("response_type=code"),
        "auth_url must declare the OAuth2 authorization-code grant, got: {url}"
    );
}

#[test]
fn auth_url_emits_pkce_plain_challenge_and_method() {
    let pkce = Pkce::new_plain();
    let url = production_provider().auth_url(&pkce, "csrf");
    assert!(
        url.contains("code_challenge_method=plain"),
        "MAL spec mandates plain — auth_url must declare it, got: {url}"
    );
    // The url crate uses `application/x-www-form-urlencoded` which
    // percent-encodes `~` even though it's a PKCE-allowed unreserved
    // char per RFC 7636. MAL's parser decodes it back so the wire
    // form is fine — assert on the post-encoding form.
    let encoded_challenge =
        url::form_urlencoded::byte_serialize(pkce.challenge.as_bytes()).collect::<String>();
    assert!(
        url.contains(&format!("code_challenge={encoded_challenge}")),
        "auth_url must carry the PKCE challenge (url-encoded), got: {url}"
    );
}

#[test]
#[should_panic(expected = "MAL requires PKCE method=plain")]
fn auth_url_panics_when_handed_an_s256_pkce() {
    // MAL's authorize endpoint rejects code_challenge_method=S256.
    // The provider hard-asserts the method instead of silently
    // emitting an S256 URL the user's browser would 400 on.
    let mut pkce = Pkce::new_plain();
    pkce.method = PkceMethod::S256;
    let _ = production_provider().auth_url(&pkce, "csrf");
}

/// Canonical MAL token response — `expires_in` in seconds (1 hour
/// per their docs), opaque access_token + refresh_token.
const MAL_TOKEN_RESPONSE_BODY: &str = r#"{
    "token_type": "Bearer",
    "expires_in": 3600,
    "access_token": "mal-access-token-xyz",
    "refresh_token": "mal-refresh-token-abc"
}"#;

#[tokio::test]
async fn exchange_code_parses_access_token_refresh_and_expiry() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(MAL_TOKEN_RESPONSE_BODY))
        .mount(&server)
        .await;
    let provider = make_provider("http://unused-api", &server.uri());
    let pkce = Pkce::new_plain();
    let now_floor = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let tokens = provider
        .exchange_code("the-auth-code", &pkce)
        .await
        .expect("exchange ok");
    assert_eq!(tokens.access_token, "mal-access-token-xyz");
    assert_eq!(
        tokens.refresh_token.as_deref(),
        Some("mal-refresh-token-abc")
    );
    // 3600s ± a few seconds of wiggle for slow CI.
    let expected_min = now_floor + 3600 - 5;
    let expected_max = now_floor + 3600 + 60;
    assert!(
        tokens.expires_at_epoch_s >= expected_min && tokens.expires_at_epoch_s <= expected_max,
        "expires_at_epoch_s ({}) must be within [{}, {}]",
        tokens.expires_at_epoch_s,
        expected_min,
        expected_max
    );
}

#[tokio::test]
async fn exchange_code_sends_form_body_with_pkce_verifier_and_no_client_secret() {
    use wiremock::matchers::body_string_contains;
    let server = wiremock::MockServer::start().await;
    let pkce = Pkce::new_plain();
    // `reqwest::Form` url-encodes the verifier; assert on the encoded
    // form so a `.`/`~`/`_` in the verifier doesn't trip the matcher.
    let encoded_verifier =
        url::form_urlencoded::byte_serialize(pkce.verifier.as_bytes()).collect::<String>();
    let verifier_marker = format!("code_verifier={encoded_verifier}");
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::header(
            "content-type",
            "application/x-www-form-urlencoded",
        ))
        .and(body_string_contains("grant_type=authorization_code"))
        .and(body_string_contains(&format!("client_id={MAL_CLIENT_ID}")))
        .and(body_string_contains("code=the-auth-code"))
        .and(body_string_contains(&verifier_marker))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(MAL_TOKEN_RESPONSE_BODY))
        .mount(&server)
        .await;
    let provider = make_provider("http://unused-api", &server.uri());
    let tokens = provider.exchange_code("the-auth-code", &pkce).await;
    assert!(
        tokens.is_ok(),
        "exchange_code must hit the wiremock matcher: {tokens:?}"
    );
}

#[tokio::test]
async fn exchange_code_surfaces_4xx_as_oauth_exchange_failed_or_upstream() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(400).set_body_string(r#"{"error":"invalid_grant"}"#),
        )
        .mount(&server)
        .await;
    let provider = make_provider("http://unused-api", &server.uri());
    let err = provider
        .exchange_code("bad-code", &Pkce::new_plain())
        .await
        .expect_err("4xx must surface as error");
    match err {
        crate::error::AniError::Upstream { status } => assert_eq!(status, 400),
        other => panic!("expected Upstream {{ status: 400 }}, got {other:?}"),
    }
}

#[tokio::test]
async fn refresh_rotates_tokens_with_form_body_carrying_refresh_token() {
    use wiremock::matchers::body_string_contains;
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::header(
            "content-type",
            "application/x-www-form-urlencoded",
        ))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=stale-refresh-xyz"))
        .and(body_string_contains(&format!("client_id={MAL_CLIENT_ID}")))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(MAL_TOKEN_RESPONSE_BODY))
        .mount(&server)
        .await;
    let provider = make_provider("http://unused-api", &server.uri());
    let tokens = provider
        .refresh("stale-refresh-xyz")
        .await
        .expect("refresh ok");
    assert_eq!(tokens.access_token, "mal-access-token-xyz");
}

#[tokio::test]
async fn refresh_surfaces_4xx_as_upstream() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let provider = make_provider("http://unused-api", &server.uri());
    let err = provider
        .refresh("revoked")
        .await
        .expect_err("4xx must surface");
    match err {
        crate::error::AniError::Upstream { status } => assert_eq!(status, 401),
        other => panic!("expected Upstream {{ status: 401 }}, got {other:?}"),
    }
}

#[tokio::test]
async fn refresh_lock_serializes_concurrent_calls() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    let server = wiremock::MockServer::start().await;
    // Slow response holds the upstream busy long enough that without
    // the mutex two concurrent calls would overlap. With the mutex
    // we'll observe two SEQUENTIAL requests, never concurrent.
    let in_flight = Arc::new(AtomicUsize::new(0));
    let max_observed = Arc::new(AtomicUsize::new(0));
    let in_flight2 = in_flight.clone();
    let max_observed2 = max_observed.clone();
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(move |_: &wiremock::Request| {
            let n = in_flight2.fetch_add(1, Ordering::SeqCst) + 1;
            max_observed2.fetch_max(n, Ordering::SeqCst);
            // 50ms gives the second caller plenty of time to race in
            // without the lock; with the lock the second call doesn't
            // start until after we leave this closure.
            std::thread::sleep(Duration::from_millis(50));
            in_flight2.fetch_sub(1, Ordering::SeqCst);
            wiremock::ResponseTemplate::new(200).set_body_string(MAL_TOKEN_RESPONSE_BODY)
        })
        .mount(&server)
        .await;
    let provider = Arc::new(make_provider("http://unused-api", &server.uri()));
    let p1 = provider.clone();
    let p2 = provider.clone();
    let h1 = tokio::spawn(async move { p1.refresh("token-1").await });
    let h2 = tokio::spawn(async move { p2.refresh("token-2").await });
    let _ = h1.await.unwrap().expect("refresh 1 ok");
    let _ = h2.await.unwrap().expect("refresh 2 ok");
    let observed = max_observed.load(Ordering::SeqCst);
    assert_eq!(
        observed, 1,
        "refresh_lock must serialize — saw {observed} concurrent in-flight calls"
    );
}

#[tokio::test]
async fn exchange_code_surfaces_5xx_as_upstream() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let provider = make_provider("http://unused-api", &server.uri());
    let err = provider
        .exchange_code("the-auth-code", &Pkce::new_plain())
        .await
        .expect_err("5xx must surface as error");
    match err {
        crate::error::AniError::Upstream { status } => assert_eq!(status, 503),
        other => panic!("expected Upstream {{ status: 503 }}, got {other:?}"),
    }
}
