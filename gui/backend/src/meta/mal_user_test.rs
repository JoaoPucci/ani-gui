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
    let url = production_provider()
        .auth_url(&plain_pkce(), "csrf-token")
        .expect("plain pkce ok");
    assert!(
        url.starts_with(MAL_AUTH_URL),
        "auth_url must point at MAL's authorize endpoint, got: {url}"
    );
}

#[test]
fn auth_url_carries_the_public_client_id() {
    let url = production_provider()
        .auth_url(&plain_pkce(), "csrf-token")
        .expect("plain pkce ok");
    assert!(
        url.contains(&format!("client_id={MAL_CLIENT_ID}")),
        "auth_url must carry the public client_id, got: {url}"
    );
}

#[test]
fn auth_url_round_trips_the_csrf_state() {
    let url = production_provider()
        .auth_url(&plain_pkce(), "csrf-token-xyz")
        .expect("plain pkce ok");
    assert!(
        url.contains("state=csrf-token-xyz"),
        "auth_url must round-trip the CSRF state, got: {url}"
    );
}

#[test]
fn auth_url_uses_the_registered_redirect_uri() {
    let url = production_provider()
        .auth_url(&plain_pkce(), "csrf")
        .expect("plain pkce ok");
    let encoded =
        url::form_urlencoded::byte_serialize(MAL_REDIRECT_URI.as_bytes()).collect::<String>();
    assert!(
        url.contains(&format!("redirect_uri={encoded}")),
        "auth_url must declare the registered redirect_uri, got: {url}"
    );
}

#[test]
fn auth_url_declares_response_type_code() {
    let url = production_provider()
        .auth_url(&plain_pkce(), "csrf")
        .expect("plain pkce ok");
    assert!(
        url.contains("response_type=code"),
        "auth_url must declare the OAuth2 authorization-code grant, got: {url}"
    );
}

#[test]
fn auth_url_emits_pkce_plain_challenge_and_method() {
    let pkce = Pkce::new_plain();
    let url = production_provider()
        .auth_url(&pkce, "csrf")
        .expect("plain pkce ok");
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
fn auth_url_returns_error_for_s256_pkce_so_route_layer_surfaces_clean_4xx() {
    // MAL's authorize endpoint rejects code_challenge_method=S256.
    // Provider returns Err(AniError::Metadata) so the auth-url route
    // handler returns a clean 4xx — never a panic (Codex P2
    // #3375623160) and never a 200 with `url: ""` that would fail
    // silently later in the connect flow (Codex P2 #3375657046).
    let mut pkce = Pkce::new_plain();
    pkce.method = PkceMethod::S256;
    let err = production_provider()
        .auth_url(&pkce, "csrf")
        .expect_err("S256 must be rejected");
    assert!(
        matches!(err, crate::error::AniError::Metadata),
        "expected AniError::Metadata, got {err:?}"
    );
}

// `parse_iso8601_to_epoch` has multiple early-return branches for
// malformed input; cover them so a single regression doesn't push
// the file's CRAP score over the ratchet.

#[test]
fn parse_iso8601_to_epoch_returns_zero_for_short_input() {
    assert_eq!(crate::meta::mal_user_parse::parse_iso8601_to_epoch(""), 0);
    assert_eq!(
        crate::meta::mal_user_parse::parse_iso8601_to_epoch("2026-01-01"),
        0
    );
}

#[test]
fn parse_iso8601_to_epoch_returns_zero_for_non_numeric_segments() {
    assert_eq!(
        crate::meta::mal_user_parse::parse_iso8601_to_epoch("XXXX-01-01T00:00:00+00:00"),
        0
    );
    assert_eq!(
        crate::meta::mal_user_parse::parse_iso8601_to_epoch("2026-XX-01T00:00:00+00:00"),
        0
    );
    assert_eq!(
        crate::meta::mal_user_parse::parse_iso8601_to_epoch("2026-01-XXT00:00:00+00:00"),
        0
    );
    assert_eq!(
        crate::meta::mal_user_parse::parse_iso8601_to_epoch("2026-01-01TXX:00:00+00:00"),
        0
    );
    assert_eq!(
        crate::meta::mal_user_parse::parse_iso8601_to_epoch("2026-01-01T00:XX:00+00:00"),
        0
    );
    assert_eq!(
        crate::meta::mal_user_parse::parse_iso8601_to_epoch("2026-01-01T00:00:XX+00:00"),
        0
    );
}

#[test]
fn parse_iso8601_to_epoch_parses_unix_epoch_origin() {
    // 1970-01-01T00:00:00 UTC = 0.
    assert_eq!(
        crate::meta::mal_user_parse::parse_iso8601_to_epoch("1970-01-01T00:00:00+00:00"),
        0
    );
}

#[test]
fn parse_iso8601_to_epoch_parses_known_timestamp() {
    // 2026-01-15T10:30:00 — same string used by the list_all happy
    // path. We just want a strictly-positive epoch, not the exact
    // value (the offset is dropped on purpose; the comparator is
    // ordering, not absolute time).
    let ts = crate::meta::mal_user_parse::parse_iso8601_to_epoch("2026-01-15T10:30:00+00:00");
    assert!(ts > 0, "parser must produce a positive epoch, got {ts}");
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
        .and(body_string_contains(format!("client_id={MAL_CLIENT_ID}")))
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

/// Canonical `/v2/users/@me?fields=anime_statistics` response.
const MAL_VIEWER_BODY: &str = r#"{
    "id": 4242,
    "name": "shiro",
    "picture": "https://cdn.myanimelist.net/images/userimages/4242.jpg",
    "anime_statistics": {
        "num_items_watching": 5,
        "num_items_completed": 100,
        "num_items_on_hold": 1,
        "num_items_dropped": 2,
        "num_items_plan_to_watch": 30,
        "mean_score": 7.5
    }
}"#;

#[tokio::test]
async fn me_parses_user_profile_and_anime_stats() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/users/@me"))
        .and(wiremock::matchers::query_param(
            "fields",
            "anime_statistics",
        ))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(MAL_VIEWER_BODY))
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let tokens = crate::account::provider::Tokens {
        access_token: "mal-access".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    let profile = provider.me(&tokens).await.expect("me ok");
    assert_eq!(profile.user_id, "4242");
    assert_eq!(profile.username, "shiro");
    assert_eq!(
        profile.avatar_url.as_deref(),
        Some("https://cdn.myanimelist.net/images/userimages/4242.jpg")
    );
    let stats = profile.stats.expect("stats present");
    assert_eq!(stats.anime_count, 138);
    assert_eq!(stats.mean_score_0_to_10, Some(7.5));
}

#[tokio::test]
async fn me_sends_x_mal_client_id_header() {
    use wiremock::matchers::header;
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/users/@me"))
        .and(header("x-mal-client-id", MAL_CLIENT_ID))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(MAL_VIEWER_BODY))
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let tokens = crate::account::provider::Tokens {
        access_token: "mal-access".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    provider
        .me(&tokens)
        .await
        .expect("me must hit the matcher demanding X-MAL-CLIENT-ID");
}

#[tokio::test]
async fn me_401_surfaces_as_invalid_token() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/users/@me"))
        .respond_with(wiremock::ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let tokens = crate::account::provider::Tokens {
        access_token: "revoked".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    match provider.me(&tokens).await {
        Err(crate::error::AniError::InvalidToken) => {}
        other => panic!("expected InvalidToken, got {other:?}"),
    }
}

const MAL_LIST_PAGE_BODY: &str = r#"{
    "data": [
        {
            "node": { "id": 21, "title": "One Piece" },
            "list_status": {
                "status": "watching",
                "score": 9,
                "num_episodes_watched": 1100,
                "is_rewatching": false,
                "updated_at": "2026-01-15T10:30:00+00:00"
            }
        },
        {
            "node": { "id": 5114, "title": "Fullmetal Alchemist: Brotherhood" },
            "list_status": {
                "status": "completed",
                "score": 10,
                "num_episodes_watched": 64,
                "is_rewatching": false,
                "updated_at": "2025-06-01T18:00:00+00:00"
            }
        }
    ],
    "paging": {}
}"#;

#[tokio::test]
async fn list_all_maps_status_score_and_progress() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/users/@me/animelist"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(MAL_LIST_PAGE_BODY))
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let tokens = crate::account::provider::Tokens {
        access_token: "mal-access".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    let entries = provider.list_all(&tokens).await.expect("list_all ok");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].media_id.0, 21);
    assert_eq!(entries[0].mal_id, Some(21));
    assert_eq!(
        entries[0].status,
        crate::account::status::ListStatus::Watching
    );
    // 9 (0..=10) → 90 (0..=100).
    assert_eq!(entries[0].score_0_to_100, Some(90));
    assert_eq!(entries[0].progress_episodes, 1100);
    assert!(entries[0].updated_at_epoch_s > 0);
    assert_eq!(
        entries[1].status,
        crate::account::status::ListStatus::Completed
    );
    assert_eq!(entries[1].score_0_to_100, Some(100));
}

#[tokio::test]
async fn list_all_follows_paging_next() {
    // First page: paging.next points at /page2; second page: empty
    // paging so the loop ends.
    let server = wiremock::MockServer::start().await;
    let page2_url = format!("{}/page2", server.uri());
    let page1_body = format!(
        r#"{{
            "data": [{{
                "node": {{ "id": 1, "title": "First" }},
                "list_status": {{
                    "status": "plan_to_watch",
                    "score": 0,
                    "num_episodes_watched": 0,
                    "is_rewatching": false,
                    "updated_at": "2026-01-01T00:00:00+00:00"
                }}
            }}],
            "paging": {{ "next": "{page2_url}" }}
        }}"#
    );
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/users/@me/animelist"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(page1_body))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/page2"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(MAL_LIST_PAGE_BODY))
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let tokens = crate::account::provider::Tokens {
        access_token: "mal-access".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    let entries = provider.list_all(&tokens).await.expect("list_all ok");
    // 1 from page1 + 2 from page2 (the canonical sample) = 3 total.
    assert_eq!(entries.len(), 3);
}

#[tokio::test]
async fn list_all_score_of_zero_means_unrated() {
    let body = r#"{
        "data": [{
            "node": { "id": 99, "title": "Unrated" },
            "list_status": {
                "status": "plan_to_watch",
                "score": 0,
                "num_episodes_watched": 0,
                "is_rewatching": false,
                "updated_at": "2026-01-01T00:00:00+00:00"
            }
        }],
        "paging": {}
    }"#;
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/users/@me/animelist"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let tokens = crate::account::provider::Tokens {
        access_token: "mal-access".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    let entries = provider.list_all(&tokens).await.expect("list_all ok");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].score_0_to_100, None);
}

#[tokio::test]
async fn list_all_drops_off_origin_paging_next() {
    // Page 1 returns `paging.next` pointing at an attacker host;
    // list_all MUST NOT follow it (would leak the bearer +
    // X-MAL-CLIENT-ID off-origin). Codex P2 #3375623170.
    let server = wiremock::MockServer::start().await;
    let body = r#"{
        "data": [{
            "node": { "id": 1, "title": "Page1" },
            "list_status": {
                "status": "plan_to_watch",
                "score": 0,
                "num_episodes_watched": 0,
                "is_rewatching": false,
                "updated_at": "2026-01-01T00:00:00+00:00"
            }
        }],
        "paging": { "next": "https://attacker.example.com/anime/list?cursor=2" }
    }"#;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/users/@me/animelist"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let tokens = crate::account::provider::Tokens {
        access_token: "mal-access".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    let entries = provider
        .list_all(&tokens)
        .await
        .expect("list_all ok — off-origin next should be dropped, not followed");
    // Only the first page lands; the attacker URL is never hit.
    assert_eq!(entries.len(), 1);
}

#[tokio::test]
async fn list_all_401_surfaces_as_invalid_token() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/users/@me/animelist"))
        .respond_with(wiremock::ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let tokens = crate::account::provider::Tokens {
        access_token: "revoked".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    match provider.list_all(&tokens).await {
        Err(crate::error::AniError::InvalidToken) => {}
        other => panic!("expected InvalidToken, got {other:?}"),
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
        .and(body_string_contains(format!("client_id={MAL_CLIENT_ID}")))
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
async fn refresh_coalesces_concurrent_calls_with_same_refresh_token() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    let server = wiremock::MockServer::start().await;
    let hit_count = Arc::new(AtomicUsize::new(0));
    let hit_count2 = hit_count.clone();
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(move |_: &wiremock::Request| {
            hit_count2.fetch_add(1, Ordering::SeqCst);
            wiremock::ResponseTemplate::new(200).set_body_string(MAL_TOKEN_RESPONSE_BODY)
        })
        .mount(&server)
        .await;
    let provider = Arc::new(make_provider("http://unused-api", &server.uri()));
    // Two concurrent callers hand over the SAME stale refresh token.
    let p1 = provider.clone();
    let p2 = provider.clone();
    let h1 = tokio::spawn(async move { p1.refresh("stale").await });
    let h2 = tokio::spawn(async move { p2.refresh("stale").await });
    let t1 = h1.await.unwrap().expect("first refresh ok");
    let t2 = h2.await.unwrap().expect("second refresh must coalesce");
    // Both callers receive the same rotated tokens.
    assert_eq!(t1.access_token, t2.access_token);
    assert_eq!(t1.refresh_token, t2.refresh_token);
    // Exactly ONE upstream POST happened — the second caller hit the
    // coalesce cache instead of re-POSTing the now-invalidated
    // refresh token.
    assert_eq!(
        hit_count.load(Ordering::SeqCst),
        1,
        "refresh must coalesce — saw {} POSTs",
        hit_count.load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn refresh_does_not_coalesce_when_cached_tokens_are_expired() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    // `expires_in: 0` → tokens are already expired the moment they
    // land in the cache. The second caller must NOT hit the cache;
    // it must do a fresh network call (which then propagates the
    // upstream's real 401 if the refresh token was invalidated by
    // the first rotation).
    let server = wiremock::MockServer::start().await;
    let hit_count = Arc::new(AtomicUsize::new(0));
    let hit_count2 = hit_count.clone();
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(move |_: &wiremock::Request| {
            hit_count2.fetch_add(1, Ordering::SeqCst);
            wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"token_type":"Bearer","expires_in":0,"access_token":"already-expired","refresh_token":"new"}"#,
            )
        })
        .mount(&server)
        .await;
    let provider = make_provider("http://unused-api", &server.uri());
    let _first = provider
        .refresh("stale")
        .await
        .expect("first refresh ok (but tokens are pre-expired)");
    let _second = provider
        .refresh("stale")
        .await
        .expect("second refresh hits the network again");
    assert_eq!(
        hit_count.load(Ordering::SeqCst),
        2,
        "expired cached tokens must NOT be returned — second refresh must re-POST"
    );
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
