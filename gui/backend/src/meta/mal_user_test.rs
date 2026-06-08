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
