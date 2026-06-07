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

/// Real-shape response to `query Viewer { Viewer { … } }`. Numeric
/// `id` (not stringy), avatar bag, statistics with anime stats
/// nested two levels deep. AniList's `meanScore` is already on the
/// 0..=10 scale here — pass through.
const VIEWER_RESPONSE_BODY: &str = r#"{
    "data": {
        "Viewer": {
            "id": 5921,
            "name": "pucci",
            "avatar": {
                "large": "https://s4.anilist.co/file/anilistcdn/user/avatar/large/b5921-x.png",
                "medium": "https://s4.anilist.co/file/anilistcdn/user/avatar/medium/b5921-x.png"
            },
            "statistics": {
                "anime": {
                    "count": 312,
                    "meanScore": 7.4
                }
            }
        }
    }
}"#;

#[tokio::test]
async fn me_parses_viewer_query_response() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/"))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer test-access-token",
        ))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(VIEWER_RESPONSE_BODY))
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let profile = provider.me(&dummy_tokens()).await.expect("me ok");

    assert_eq!(profile.provider, ProviderKind::AniList);
    // AniList ids are numeric on the wire; UserProfile.user_id is a
    // String so the same shape fits MAL's "@me" id later.
    assert_eq!(profile.user_id, "5921");
    assert_eq!(profile.username, "pucci");
    assert_eq!(
        profile.avatar_url.as_deref(),
        Some("https://s4.anilist.co/file/anilistcdn/user/avatar/large/b5921-x.png")
    );
    let stats = profile.stats.expect("stats present");
    assert_eq!(stats.anime_count, 312);
    // meanScore is already 0..=10; pass through, no scaling.
    assert!(
        matches!(stats.mean_score_0_to_10, Some(v) if (v - 7.4).abs() < 0.001),
        "mean_score_0_to_10: {:?}",
        stats.mean_score_0_to_10
    );
}

#[tokio::test]
async fn me_surfaces_401_as_invalid_token() {
    // A 401 on the Viewer query means the bearer is bad — almost
    // always because the user revoked the app from anilist.co or
    // (rare) because the 1-year JWT silently expired. The route
    // layer surfaces this distinctly from generic Upstream so the
    // /account page can show "Sign in again" instead of a retry.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(401)
                .set_body_string(r#"{"errors":[{"message":"Invalid token"}]}"#),
        )
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let err = provider
        .me(&dummy_tokens())
        .await
        .expect_err("401 must surface as Err");
    assert!(
        matches!(err, AniError::InvalidToken),
        "expected AniError::InvalidToken, got {err:?}"
    );
}

#[tokio::test]
async fn refresh_returns_metadata_error_no_network_call() {
    // AniList does not issue refresh tokens — their 1-year JWT has
    // no refresh flow and no revocation endpoint. The trait method
    // must surface this as AniError::Metadata so the route layer can
    // distinguish "no flow" from a transient upstream / network
    // failure (which would be Network / Upstream).
    //
    // Build the provider with bogus endpoints to also prove the impl
    // doesn't attempt a network round-trip — any HTTP call against
    // these unbound URIs would surface as Network, not Metadata.
    let provider = AniListProvider::with_bases(
        reqwest::Client::new(),
        "http://127.0.0.1:1/no-api".into(),
        "http://127.0.0.1:1/no-token".into(),
    );
    let err = provider
        .refresh("never-issued")
        .await
        .expect_err("refresh must always error");
    assert!(
        matches!(err, AniError::Metadata),
        "expected AniError::Metadata, got {err:?}"
    );
}
