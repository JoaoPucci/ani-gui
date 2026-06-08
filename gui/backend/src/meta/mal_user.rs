//! MyAnimeList implementation of [`UserListProvider`].
//!
//! Mirrors the AniList provider's shape (`new` / `with_bases` so tests
//! mount wiremock; `kind` / `auth_url` / `exchange_code` / `me` /
//! `list_all` / write methods from the trait) and adds two MAL-specific
//! concerns the AniList provider doesn't need:
//!
//! 1. **PKCE is mandatory and `plain` only.** MAL's OAuth docs explicitly
//!    forbid `S256` — the `code_challenge_method` query parameter must
//!    be `plain`. The PKCE helper is constructed by the caller; this
//!    provider asserts the method at `auth_url` so a future caller can't
//!    accidentally hand over an `S256` pair.
//!
//! 2. **Pre-emptive token refresh.** MAL access tokens last 1 hour;
//!    refresh tokens last 1 month. Every handler call checks expiry and
//!    rotates within a 5-minute lead so a long-running list page doesn't
//!    401 mid-stream. Concurrent refresh attempts are serialized by a
//!    per-instance `tokio::sync::Mutex` — without that, two parallel
//!    handler calls both POST `/v1/oauth2/token`, one of the refresh
//!    tokens is invalidated, and the next request 401s.
//!
//! Endpoints (overridable for tests via [`MalProvider::with_bases`]):
//!
//! - `https://api.myanimelist.net/v2` — data API (anime list + user)
//! - `https://myanimelist.net/v1/oauth2/token` — OAuth token exchange
//! - `https://myanimelist.net/v1/oauth2/authorize` — browser-side
//!   authorize URL (not hit by the backend; rendered into `auth_url`)
//!
//! Every API request must carry `X-MAL-CLIENT-ID` per the App Type
//! "Other" auth model — the bearer alone is rejected.

use async_trait::async_trait;

use crate::account::credentials::{
    MAL_API, MAL_AUTH_URL, MAL_CLIENT_ID, MAL_REDIRECT_URI, MAL_TOKEN_URL,
};
use crate::account::pkce::{Pkce, PkceMethod};
use crate::account::provider::{
    EntryUpdate, ListEntry, ProviderKind, ProviderMediaId, Tokens, UserListProvider, UserProfile,
};
use crate::error::{AniError, Result};

/// `User-Agent` advertised on every MAL request. Per the API license
/// notes (Phase 0), we identify clearly so MAL can correlate traffic if
/// they ever audit.
#[allow(dead_code)] // Wired in once the network methods land.
const MAL_USER_AGENT: &str = concat!("ani-gui/", env!("CARGO_PKG_VERSION"));

/// MyAnimeList implementation of [`UserListProvider`].
///
/// Two endpoint overrides — `api_base` for the v2 data endpoint and
/// `token_base` for the OAuth token-exchange endpoint — let tests point
/// at wiremock while production hits the real `myanimelist.net`.
pub struct MalProvider {
    #[allow(dead_code)] // Wired in once me/list_all/refresh land.
    client: reqwest::Client,
    /// Override for the v2 data endpoint. `None` → production
    /// [`MAL_API`]. Tests pass a wiremock URI.
    #[allow(dead_code)] // Wired in once me/list_all land.
    api_base: Option<String>,
    /// Override for the OAuth token endpoint. `None` → production
    /// [`MAL_TOKEN_URL`]. Tests pass a wiremock URI.
    #[allow(dead_code)] // Wired in once exchange_code / refresh land.
    token_base: Option<String>,
}

impl MalProvider {
    /// Build a provider that hits production MAL endpoints.
    #[must_use]
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            api_base: None,
            token_base: None,
        }
    }

    /// Build a provider with wiremock-style endpoint overrides — the
    /// test harness mounts mock responses on these URIs.
    #[must_use]
    pub fn with_bases(client: reqwest::Client, api_base: String, token_base: String) -> Self {
        Self {
            client,
            api_base: Some(api_base),
            token_base: Some(token_base),
        }
    }

    #[allow(dead_code)] // Wired in once me/list_all land.
    fn api_url(&self) -> &str {
        self.api_base.as_deref().unwrap_or(MAL_API)
    }

    #[allow(dead_code)] // Wired in once exchange_code / refresh land.
    fn token_url(&self) -> &str {
        self.token_base.as_deref().unwrap_or(MAL_TOKEN_URL)
    }
}

#[async_trait]
impl UserListProvider for MalProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::MyAnimeList
    }

    fn auth_url(&self, pkce: &Pkce, state: &str) -> String {
        // MAL's authorize endpoint rejects S256 — the docs explicitly
        // require `plain`. Hard-assert at the boundary so a future
        // caller can't silently emit an S256 URL the browser would
        // 400 on. The PKCE helper has separate `new_plain` and
        // `new_s256` constructors for symmetric trait callers, but
        // for MAL only the plain variant is legal on the wire.
        assert!(
            matches!(pkce.method, PkceMethod::Plain),
            "MAL requires PKCE method=plain (S256 forbidden by spec)"
        );
        let params = [
            ("response_type", "code"),
            ("client_id", MAL_CLIENT_ID),
            ("redirect_uri", MAL_REDIRECT_URI),
            ("state", state),
            ("code_challenge", pkce.challenge.as_str()),
            ("code_challenge_method", pkce.method.as_param()),
        ];
        url::Url::parse_with_params(MAL_AUTH_URL, &params)
            .map(String::from)
            .unwrap_or_default()
    }

    async fn exchange_code(&self, _code: &str, _pkce: &Pkce) -> Result<Tokens> {
        Err(AniError::Metadata)
    }

    async fn refresh(&self, _refresh_token: &str) -> Result<Tokens> {
        Err(AniError::Metadata)
    }

    async fn me(&self, _tokens: &Tokens) -> Result<UserProfile> {
        Err(AniError::Metadata)
    }

    async fn list_all(&self, _tokens: &Tokens) -> Result<Vec<ListEntry>> {
        Err(AniError::Metadata)
    }

    async fn update_entry(
        &self,
        _tokens: &Tokens,
        _id: ProviderMediaId,
        _update: EntryUpdate,
    ) -> Result<ListEntry> {
        Err(AniError::Metadata)
    }

    async fn delete_entry(&self, _tokens: &Tokens, _id: ProviderMediaId) -> Result<()> {
        Err(AniError::Metadata)
    }
}

#[cfg(test)]
#[path = "mal_user_test.rs"]
mod tests;
