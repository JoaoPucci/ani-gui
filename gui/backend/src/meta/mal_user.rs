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
use tokio::sync::Mutex;

use super::mal_user_parse::{parse_list_page, parse_token_response, parse_viewer_response};
use crate::account::credentials::{
    MAL_API, MAL_AUTH_URL, MAL_CLIENT_ID, MAL_REDIRECT_URI, MAL_TOKEN_URL,
};
use crate::account::pkce::{Pkce, PkceMethod};
use crate::account::provider::{
    EntryUpdate, ListEntry, ProviderKind, ProviderMediaId, Tokens, UserListProvider, UserProfile,
};
use crate::error::{AniError, Result};

/// Page size for `/v2/users/@me/animelist`. MAL caps at 1000; we
/// request the cap so a heavy listmaker resolves in one or two
/// round-trips.
const MAL_LIST_PAGE_LIMIT: u32 = 1000;

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
    #[allow(dead_code)] // Wired in once me/list_all land.
    client: reqwest::Client,
    /// Override for the v2 data endpoint. `None` → production
    /// [`MAL_API`]. Tests pass a wiremock URI.
    #[allow(dead_code)] // Wired in once me/list_all land.
    api_base: Option<String>,
    /// Override for the OAuth token endpoint. `None` → production
    /// [`MAL_TOKEN_URL`]. Tests pass a wiremock URI.
    token_base: Option<String>,
    /// Serializes + coalesces concurrent `refresh` calls. Two
    /// parallel handler calls would otherwise both POST
    /// `/v1/oauth2/token` and rotate the refresh token; the first
    /// rotation invalidates the second caller's stale refresh
    /// token, and the second 401s. The mutex makes the calls
    /// sequential AND caches the last successful rotation — when a
    /// second caller arrives holding the SAME input refresh token
    /// the cache hit returns the first caller's result without a
    /// second network round trip (Codex P2 #3375519102).
    last_refresh: Mutex<Option<CoalescedRefresh>>,
}

/// Cache slot for the last successful refresh, keyed by the input
/// refresh token. Lets concurrent refreshers share one upstream
/// rotation instead of each invalidating the previous result.
struct CoalescedRefresh {
    input_refresh_token: String,
    tokens: Tokens,
}

impl MalProvider {
    /// Build a provider that hits production MAL endpoints.
    #[must_use]
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            api_base: None,
            token_base: None,
            last_refresh: Mutex::new(None),
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
            last_refresh: Mutex::new(None),
        }
    }

    #[allow(dead_code)] // Wired in once me/list_all land.
    fn api_url(&self) -> &str {
        self.api_base.as_deref().unwrap_or(MAL_API)
    }

    fn token_url(&self) -> &str {
        self.token_base.as_deref().unwrap_or(MAL_TOKEN_URL)
    }

    /// Shared form-encoded POST to MAL's OAuth token endpoint. Both
    /// `exchange_code` and `refresh` use it — only the form body
    /// differs. Returns parsed `Tokens` on 2xx, `AniError::Upstream`
    /// for non-2xx, `AniError::Network` for transport failures.
    async fn post_token_form(&self, form: &[(&str, &str)]) -> Result<Tokens> {
        let resp = self
            .client
            .post(self.token_url())
            .header("user-agent", MAL_USER_AGENT)
            .form(form)
            .send()
            .await
            .map_err(|_| AniError::Network)?;
        let status = resp.status();
        if !status.is_success() {
            return Err(AniError::Upstream {
                status: status.as_u16(),
            });
        }
        let bytes = resp.bytes().await.map_err(|_| AniError::Network)?;
        parse_token_response(&bytes)
    }

    /// Shared GET that attaches the bearer + the mandatory
    /// `X-MAL-CLIENT-ID` header. Used by `me` and `list_all` (and
    /// any future read endpoint).
    async fn get_auth_bytes(&self, url: &str, tokens: &Tokens) -> Result<bytes::Bytes> {
        let resp = self
            .client
            .get(url)
            .header("user-agent", MAL_USER_AGENT)
            .header("x-mal-client-id", MAL_CLIENT_ID)
            .bearer_auth(&tokens.access_token)
            .send()
            .await
            .map_err(|_| AniError::Network)?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AniError::InvalidToken);
        }
        if !status.is_success() {
            return Err(AniError::Upstream {
                status: status.as_u16(),
            });
        }
        resp.bytes().await.map_err(|_| AniError::Network)
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

    async fn exchange_code(&self, code: &str, pkce: &Pkce) -> Result<Tokens> {
        // MAL's token endpoint takes `application/x-www-form-urlencoded`
        // and — uniquely for App Type "Other" — has no client_secret.
        // PKCE is authentication, so `code_verifier` is required.
        let form = [
            ("client_id", MAL_CLIENT_ID),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", pkce.verifier.as_str()),
            ("redirect_uri", MAL_REDIRECT_URI),
        ];
        self.post_token_form(&form).await
    }

    async fn refresh(&self, refresh_token: &str) -> Result<Tokens> {
        // Hold the mutex across the cache-check + network call so two
        // concurrent refreshers serialize. Inside the critical
        // section: if the cache already has tokens from a previous
        // rotation of THIS refresh_token AND those tokens are still
        // live, return them — the upstream already invalidated the
        // input token; a second POST would 401. If the cached access
        // token has expired (renderer didn't persist the new
        // refresh token after the first rotation, and the backend
        // process stayed alive past expiry) we fall through to a
        // fresh network call so the caller gets the real upstream
        // response — almost certainly a 401 since `refresh_token`
        // was invalidated by the first rotation, which the caller
        // surfaces as "Sign in again" instead of looping on stale
        // tokens. Codex P2 #3375578767.
        let mut guard = self.last_refresh.lock().await;
        if let Some(cached) = guard.as_ref() {
            if cached.input_refresh_token == refresh_token {
                let now_s = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                if cached.tokens.expires_at_epoch_s > now_s {
                    return Ok(cached.tokens.clone());
                }
            }
        }
        let form = [
            ("client_id", MAL_CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];
        let tokens = self.post_token_form(&form).await?;
        *guard = Some(CoalescedRefresh {
            input_refresh_token: refresh_token.to_string(),
            tokens: tokens.clone(),
        });
        Ok(tokens)
    }

    async fn me(&self, tokens: &Tokens) -> Result<UserProfile> {
        // MAL's `/v2/users/@me` returns user fields + an optional
        // `anime_statistics` section. We always request the statistics
        // so the popover can show counts + mean score without a
        // second round trip.
        let url = format!("{}/users/@me?fields=anime_statistics", self.api_url());
        let bytes = self.get_auth_bytes(&url, tokens).await?;
        parse_viewer_response(&bytes)
    }

    async fn list_all(&self, tokens: &Tokens) -> Result<Vec<ListEntry>> {
        // MAL paginates with a fully-qualified `paging.next` URL; the
        // initial request goes to our api_url + the query string we
        // build, then each subsequent request uses whatever URL the
        // upstream handed back.
        let initial = format!(
            "{}/users/@me/animelist?fields=list_status&limit={MAL_LIST_PAGE_LIMIT}&nsfw=true",
            self.api_url()
        );
        let mut next_url = Some(initial);
        let mut out: Vec<ListEntry> = Vec::new();
        while let Some(url) = next_url.take() {
            let bytes = self.get_auth_bytes(&url, tokens).await?;
            let page = parse_list_page(&bytes)?;
            out.extend(page.entries);
            next_url = page.next_url;
        }
        Ok(out)
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
