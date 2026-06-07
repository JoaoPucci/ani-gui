//! AniList concrete [`UserListProvider`] implementation — OAuth +
//! `Viewer` profile + `MediaListCollection` user-list fetch.
//!
//! Reads only in PR #1. Write-back (`update_entry`, `delete_entry`)
//! lands in PR #4; the stubs here return [`AniError::Metadata`] until
//! then.
//!
//! AniList specifics worth keeping next to the code:
//!
//! - GraphQL endpoint: a single POST to `graphql.anilist.co`. Existing
//!   trending client lives at [`crate::meta::anilist`].
//! - OAuth: code-grant exchange returns a 1-year JWT; AniList does NOT
//!   issue refresh tokens, so [`AniListProvider::refresh`] is a hard
//!   `Err(AniError::Metadata)`. Disconnect = drop the token locally.
//! - PKCE: AniList ignores `code_challenge` / `code_challenge_method`.
//!   The trait still hands us a [`Pkce`] for symmetry with MAL; we
//!   never put it on the wire.
//! - User agent: matches the existing convention from
//!   [`crate::meta::anilist`] so any UA-based ratelimit treats both
//!   surfaces as the same client.
//! - Score scale: AniList's POINT_100 system is already 0..=100, which
//!   matches the unified scale; pass through with no conversion.

use async_trait::async_trait;
use serde::Deserialize;

use crate::account::credentials::{
    ANILIST_API, ANILIST_AUTH_URL, ANILIST_CLIENT_ID, ANILIST_CLIENT_SECRET, ANILIST_REDIRECT_URI,
    ANILIST_TOKEN_URL,
};
use crate::account::pkce::Pkce;
use crate::account::provider::{
    EntryUpdate, ListEntry, ProviderKind, ProviderMediaId, Tokens, UserListProvider, UserProfile,
};
use crate::error::{AniError, Result};

/// AniList implementation of [`UserListProvider`].
///
/// Two endpoint overrides — `api_base` for the GraphQL endpoint and
/// `token_base` for the OAuth token-exchange endpoint — let tests
/// point at wiremock while production hits the real `anilist.co`.
pub struct AniListProvider {
    client: reqwest::Client,
    /// Override for the GraphQL endpoint. `None` → production
    /// [`ANILIST_API`]. Tests pass a wiremock URI.
    api_base: Option<String>,
    /// Override for the OAuth token endpoint. `None` → production
    /// [`ANILIST_TOKEN_URL`]. Tests pass a wiremock URI.
    token_base: Option<String>,
}

impl AniListProvider {
    /// Build a provider that hits production AniList endpoints.
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

    fn api_url(&self) -> &str {
        self.api_base.as_deref().unwrap_or(ANILIST_API)
    }

    fn token_url(&self) -> &str {
        self.token_base.as_deref().unwrap_or(ANILIST_TOKEN_URL)
    }
}

#[async_trait]
impl UserListProvider for AniListProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::AniList
    }

    fn auth_url(&self, _pkce: &Pkce, state: &str) -> String {
        // AniList ignores PKCE entirely — we deliberately do not emit
        // `code_challenge` / `code_challenge_method` here. The trait
        // still hands us a `Pkce` so the MAL impl can mirror the
        // signature, but on the wire we'd only confuse AniList's
        // parser. See module doc-comment.
        //
        // `url::Url::parse_with_params` percent-encodes the values
        // per `application/x-www-form-urlencoded` — same encoding
        // AniList's authorize endpoint expects.
        let params = [
            ("client_id", ANILIST_CLIENT_ID),
            ("redirect_uri", ANILIST_REDIRECT_URI),
            ("response_type", "code"),
            ("state", state),
        ];
        url::Url::parse_with_params(ANILIST_AUTH_URL, &params)
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
#[path = "anilist_user_test.rs"]
mod tests;
