use super::*;
use crate::account::pkce::Pkce;

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
