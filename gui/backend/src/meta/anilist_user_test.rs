use super::*;
use crate::account::pkce::Pkce;
use crate::error::AniError;

/// Build the wiremock-backed provider used across the network tests.
/// `api_uri` / `token_uri` are wiremock server URIs.
fn make_provider(api_uri: &str, token_uri: &str) -> AniListProvider {
    AniListProvider::with_bases(
        reqwest::Client::new(),
        api_uri.to_string(),
        token_uri.to_string(),
    )
}

/// Tokens with a non-zero `expires_at_epoch_s` so the placeholder
/// passes the trait's expiry checks. Tests that exercise the network
/// layer don't care about the actual expiry value.
fn dummy_tokens() -> Tokens {
    Tokens {
        access_token: "test-access-token".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

/// AniList's consent URL is fixed-shape — the `state` we generate has
/// to round-trip through the browser back into our loopback handler,
/// so the URL must carry it verbatim. Pin the query keys + literal
/// values (client_id, redirect_uri, response_type) too because a
/// silent rename would make the live OAuth flow fail without the
/// trait surface changing.
#[test]
fn auth_url_builds_anilist_consent_url_with_state() {
    let provider = AniListProvider::new(reqwest::Client::new());
    // AniList ignores PKCE entirely, but the trait shape passes one in
    // for symmetry with MAL. Build a plain pair just so we have one.
    let pkce = Pkce::new_plain();
    let url = provider.auth_url(&pkce, "csrf-token-xyz");

    assert!(
        url.starts_with("https://anilist.co/api/v2/oauth/authorize?"),
        "auth_url must point at AniList's authorize endpoint: {url}"
    );
    assert!(
        url.contains("client_id=43143"),
        "auth_url must include the configured client_id: {url}"
    );
    assert!(
        url.contains("response_type=code"),
        "auth_url must request the code grant: {url}"
    );
    assert!(
        url.contains("state=csrf-token-xyz"),
        "auth_url must carry the state verbatim for CSRF round-trip: {url}"
    );
    // AniList allows exactly one redirect URI per app, so any mismatch
    // here surfaces as a hard upstream rejection. URL-encoded form is
    // what reqwest's url crate produces.
    assert!(
        url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A53682%2Fcallback"),
        "auth_url must include the encoded loopback redirect_uri: {url}"
    );
    // AniList ignores PKCE — pin that we do NOT advertise a challenge,
    // so a future "fix" that copies MAL's PKCE handling doesn't sneak
    // an invalid param into the live URL.
    assert!(
        !url.contains("code_challenge"),
        "auth_url must NOT advertise PKCE (AniList ignores it): {url}"
    );
}

/// Real shape from AniList's documented token-exchange response. The
/// 1-year `expires_in` is what makes their refresh flow superfluous;
/// no `refresh_token` field exists in the doc'd response either.
const TOKEN_RESPONSE_BODY: &str = r#"{
    "token_type": "Bearer",
    "expires_in": 31536000,
    "access_token": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.test-access-token",
    "refresh_token": null
}"#;

#[tokio::test]
async fn exchange_code_parses_access_token_and_expiry() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": "43143",
            "client_secret": "B5ay8XNwslA819aukVNC8ejQBvUXtpuLiPf1yvL7",
            "redirect_uri": "http://localhost:53682/callback",
            "code": "the-auth-code"
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(TOKEN_RESPONSE_BODY))
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
    assert_eq!(
        tokens.access_token,
        "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.test-access-token"
    );
    // AniList doesn't return a refresh_token; trait carries None.
    assert!(tokens.refresh_token.is_none());
    // expires_at must be ~now + 1 year. Allow a generous lower-bound
    // window so a slow CI runner doesn't flake. Upper bound catches
    // a future bug where the conversion accidentally double-counts.
    let expected_min = now_floor + 31_536_000 - 5;
    let expected_max = now_floor + 31_536_000 + 60;
    assert!(
        tokens.expires_at_epoch_s >= expected_min && tokens.expires_at_epoch_s <= expected_max,
        "expires_at_epoch_s ({}) must be within [{}, {}]",
        tokens.expires_at_epoch_s,
        expected_min,
        expected_max
    );
}

#[tokio::test]
async fn exchange_code_surfaces_upstream_4xx_as_upstream_error() {
    // AniList returns 400 on a stale / replayed code with a JSON
    // error body; the trait surface collapses that to Upstream so
    // the route layer can map it to a re-auth prompt.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(400).set_body_string(r#"{"error":"invalid_request"}"#),
        )
        .mount(&server)
        .await;
    let provider = make_provider("http://unused-api", &server.uri());
    let err = provider
        .exchange_code("stale-code", &Pkce::new_plain())
        .await
        .expect_err("400 must surface as Err");
    assert!(
        matches!(err, AniError::Upstream { status: 400 }),
        "expected Upstream {{ status: 400 }}, got {err:?}"
    );
    // Suppress dead-code warnings until later pairs land.
    let _ = dummy_tokens();
}
