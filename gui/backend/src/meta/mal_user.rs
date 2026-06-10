//! MyAnimeList implementation of [`UserListProvider`].
//!
//! Trait-impl shell. Mirrors the AniList provider's shape (`new` /
//! `with_bases` so tests mount wiremock; `kind` / `auth_url` /
//! `exchange_code` / `me` / `list_all` / write methods from the
//! trait). Two MAL-specific concerns the AniList provider doesn't
//! need:
//!
//! 1. **PKCE is mandatory and `plain` only.** MAL's OAuth docs
//!    explicitly forbid `S256`. The trait method returns
//!    `Err(AniError::UnsupportedPkce)` (mapped to 400 in the route
//!    layer) for any non-`Plain` PKCE.
//! 2. **Pre-emptive token refresh with coalesce.** Concurrent
//!    refreshers serialize at a `tokio::sync::Mutex` AND share the
//!    last successful rotation's tokens when the input refresh token
//!    matches and the cached access token is still live.
//!
//! Network plumbing (post_token_form, get_auth_bytes, refresh_inner,
//! url_origin, CoalescedRefresh) and the wire-parsers (parse_*)
//! live in sibling modules `mal_user_net.rs` and `mal_user_parse.rs`
//! so this file stays focused on the trait dispatch.

use async_trait::async_trait;

use super::mal_user_net::url_origin;
pub use super::mal_user_net::MalRefreshState;
use super::mal_user_parse::{
    parse_list_page, parse_list_status_response, parse_my_list_status_progress,
    parse_viewer_response,
};
use crate::account::credentials::{
    MAL_API, MAL_AUTH_URL, MAL_CLIENT_ID, MAL_REDIRECT_URI, MAL_TOKEN_URL,
};
use crate::account::pkce::{Pkce, PkceMethod};
use crate::account::provider::{
    EntryUpdate, ListEntry, ProviderKind, ProviderMediaId, Tokens, UserListProvider, UserProfile,
};
use crate::error::{AniError, Result};

/// Page size for `/v2/users/@me/animelist`. MAL caps at 1000.
const MAL_LIST_PAGE_LIMIT: u32 = 1000;

/// MyAnimeList implementation of [`UserListProvider`].
pub struct MalProvider {
    client: reqwest::Client,
    api_base: Option<String>,
    token_base: Option<String>,
    refresh_state: MalRefreshState,
}

impl MalProvider {
    /// Build a provider that hits production MAL endpoints. The
    /// `refresh_state` is shared across every provider instance the
    /// dispatcher constructs so two concurrent handler calls hit the
    /// same coalesce cache (Codex P2 #3379969316).
    #[must_use]
    pub fn new(client: reqwest::Client, refresh_state: MalRefreshState) -> Self {
        Self {
            client,
            api_base: None,
            token_base: None,
            refresh_state,
        }
    }

    /// Build a provider with wiremock-style endpoint overrides — the
    /// test harness mounts mock responses on these URIs.
    #[must_use]
    pub fn with_bases(
        client: reqwest::Client,
        api_base: String,
        token_base: String,
        refresh_state: MalRefreshState,
    ) -> Self {
        Self {
            client,
            api_base: Some(api_base),
            token_base: Some(token_base),
            refresh_state,
        }
    }

    pub(super) fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub(super) fn api_url(&self) -> &str {
        self.api_base.as_deref().unwrap_or(MAL_API)
    }

    pub(super) fn token_url(&self) -> &str {
        self.token_base.as_deref().unwrap_or(MAL_TOKEN_URL)
    }

    pub(super) fn refresh_state(&self) -> &MalRefreshState {
        &self.refresh_state
    }
}

#[async_trait]
impl UserListProvider for MalProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::MyAnimeList
    }

    fn auth_url(&self, pkce: &Pkce, state: &str) -> Result<String> {
        // MAL's authorize endpoint rejects S256. Return
        // `UnsupportedPkce` (mapped to 400 by the route layer, Codex
        // P2 #3377294701) so a renderer/local client sending the bad
        // value gets a clean request-validation failure.
        if !matches!(pkce.method, PkceMethod::Plain) {
            return Err(AniError::UnsupportedPkce);
        }
        let params = [
            ("response_type", "code"),
            ("client_id", MAL_CLIENT_ID),
            ("redirect_uri", MAL_REDIRECT_URI),
            ("state", state),
            ("code_challenge", pkce.challenge.as_str()),
            ("code_challenge_method", pkce.method.as_param()),
        ];
        Ok(url::Url::parse_with_params(MAL_AUTH_URL, &params)
            .map(String::from)
            .unwrap_or_default())
    }

    async fn exchange_code(&self, code: &str, pkce: &Pkce) -> Result<Tokens> {
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
        self.refresh_inner(refresh_token).await
    }

    async fn me(&self, tokens: &Tokens) -> Result<UserProfile> {
        let url = format!("{}/users/@me?fields=anime_statistics", self.api_url());
        let bytes = self.get_auth_bytes(&url, tokens).await?;
        parse_viewer_response(&bytes)
    }

    async fn list_all(&self, tokens: &Tokens) -> Result<Vec<ListEntry>> {
        let initial = format!(
            "{}/users/@me/animelist?fields=list_status&limit={MAL_LIST_PAGE_LIMIT}&nsfw=true",
            self.api_url()
        );
        let api_origin = url_origin(self.api_url());
        let mut next_url = Some(initial);
        let mut out: Vec<ListEntry> = Vec::new();
        while let Some(url) = next_url.take() {
            let bytes = self.get_auth_bytes(&url, tokens).await?;
            let page = parse_list_page(&bytes)?;
            out.extend(page.entries);
            // Drop off-origin paging.next so a compromised upstream
            // can't redirect pagination with our bearer attached
            // (Codex P2 #3375623170).
            next_url = page.next_url.filter(|n| url_origin(n) == api_origin);
        }
        Ok(out)
    }

    async fn update_entry(
        &self,
        tokens: &Tokens,
        id: ProviderMediaId,
        update: EntryUpdate,
    ) -> Result<ListEntry> {
        // MAL splits `Rewatching` across `status="watching"` +
        // `is_rewatching=true`; the helper carries both halves.
        let mut form: Vec<(&str, String)> = Vec::new();
        if let Some(status) = update.status {
            let (mal_status, is_rewatching) = status.to_mal();
            form.push(("status", mal_status.to_string()));
            form.push(("is_rewatching", is_rewatching.to_string()));
        }
        if let Some(progress) = update.progress_episodes {
            form.push(("num_watched_episodes", progress.to_string()));
        }
        if let Some(score_0_to_100) = update.score_0_to_100 {
            // Unified scale 0..=100 → MAL 0..=10. Integer divide so a
            // 95 round-trips as a 9 (MAL's UI shows integer tenths
            // only; finer precision is lost on display anyway).
            let mal_score = score_0_to_100 / 10;
            form.push(("score", mal_score.to_string()));
        }
        if let Some(repeat) = update.repeat_count {
            form.push(("num_times_rewatched", repeat.to_string()));
        }
        let url = format!("{}/anime/{}/my_list_status", self.api_url(), id.0);
        let bytes = self.patch_form(&url, tokens, &form).await?;
        parse_list_status_response(&bytes, id.0)
    }

    async fn delete_entry(&self, tokens: &Tokens, id: ProviderMediaId) -> Result<()> {
        let url = format!("{}/anime/{}/my_list_status", self.api_url(), id.0);
        self.delete_auth(&url, tokens).await
    }

    async fn current_progress(&self, tokens: &Tokens, id: ProviderMediaId) -> Result<Option<u32>> {
        let url = format!("{}/anime/{}?fields=my_list_status", self.api_url(), id.0);
        let bytes = self.get_auth_bytes(&url, tokens).await?;
        parse_my_list_status_progress(&bytes)
    }
}

#[cfg(test)]
#[path = "mal_user_test.rs"]
mod tests;
