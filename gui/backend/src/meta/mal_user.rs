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

use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::account::credentials::{
    MAL_API, MAL_AUTH_URL, MAL_CLIENT_ID, MAL_REDIRECT_URI, MAL_TOKEN_URL,
};
use crate::account::pkce::{Pkce, PkceMethod};
use crate::account::provider::{
    EntryUpdate, ListEntry, ProviderKind, ProviderMediaId, Tokens, UserListProvider, UserProfile,
};
use crate::account::status::ListStatus;
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
    /// Serializes concurrent `refresh` calls so two parallel handler
    /// calls don't both POST `/v1/oauth2/token` and rotate the
    /// refresh token — one of the responses would invalidate the
    /// other and the next request 401s. Plan §6 / TDD pair 3.
    refresh_lock: Mutex<()>,
}

impl MalProvider {
    /// Build a provider that hits production MAL endpoints.
    #[must_use]
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            api_base: None,
            token_base: None,
            refresh_lock: Mutex::new(()),
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
            refresh_lock: Mutex::new(()),
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
        let resp = self
            .client
            .post(self.token_url())
            .header("user-agent", MAL_USER_AGENT)
            .form(&form)
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

    async fn refresh(&self, refresh_token: &str) -> Result<Tokens> {
        // The mutex serializes concurrent refreshes so the upstream
        // never sees two simultaneous rotation requests — one rotation
        // would invalidate the other and the next API call 401s.
        let _guard = self.refresh_lock.lock().await;
        let form = [
            ("client_id", MAL_CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];
        let resp = self
            .client
            .post(self.token_url())
            .header("user-agent", MAL_USER_AGENT)
            .form(&form)
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

    async fn me(&self, tokens: &Tokens) -> Result<UserProfile> {
        // MAL's `/v2/users/@me` returns user fields + an optional
        // `anime_statistics` section. We always request the statistics
        // so the popover can show counts + mean score without a
        // second round trip.
        let url = format!("{}/users/@me?fields=anime_statistics", self.api_url());
        let resp = self
            .client
            .get(&url)
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
        let bytes = resp.bytes().await.map_err(|_| AniError::Network)?;
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
            let resp = self
                .client
                .get(&url)
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
            let bytes = resp.bytes().await.map_err(|_| AniError::Network)?;
            let page = parse_list_page(&bytes)?;
            for entry in page.entries {
                out.push(entry);
            }
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

/// Parse MAL's OAuth token-exchange response into [`Tokens`]. Both
/// `exchange_code` and (in the next pair) `refresh` use this — the
/// wire shape is identical between the initial grant and refresh
/// responses.
fn parse_token_response(body: &[u8]) -> Result<Tokens> {
    #[derive(Deserialize)]
    struct Wire {
        access_token: String,
        #[serde(default)]
        refresh_token: Option<String>,
        #[serde(default)]
        expires_in: Option<i64>,
    }
    let wire: Wire = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("mal token response: {e}"),
    })?;
    let now_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // MAL always sends expires_in; fall back to 1 hour (their stated
    // ceiling) so a missing field doesn't cause an immediate-expiry
    // disconnect on the next handler call.
    let expires_at_epoch_s = now_s + wire.expires_in.unwrap_or(3600);
    Ok(Tokens {
        access_token: wire.access_token,
        refresh_token: wire.refresh_token,
        expires_at_epoch_s,
    })
}

/// Parse MAL's `/v2/users/@me?fields=anime_statistics` response into
/// the unified [`UserProfile`]. Mean score is rescaled from MAL's
/// 0..=10 wire scale to the unified 0..=10 (no scale change for MAL).
fn parse_viewer_response(body: &[u8]) -> Result<UserProfile> {
    #[derive(Deserialize)]
    struct Wire {
        id: u64,
        name: String,
        #[serde(default)]
        picture: Option<String>,
        #[serde(default)]
        anime_statistics: Option<AnimeStats>,
    }
    #[derive(Deserialize)]
    struct AnimeStats {
        #[serde(default)]
        num_items: Option<u32>,
        #[serde(default)]
        num_items_completed: Option<u32>,
        #[serde(default)]
        num_items_watching: Option<u32>,
        #[serde(default)]
        num_items_on_hold: Option<u32>,
        #[serde(default)]
        num_items_dropped: Option<u32>,
        #[serde(default)]
        num_items_plan_to_watch: Option<u32>,
        #[serde(default)]
        mean_score: Option<f32>,
    }
    let wire: Wire = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("mal viewer response: {e}"),
    })?;
    let stats = wire.anime_statistics.map(|a| {
        // MAL exposes per-status counts but not a `num_items` total in
        // every response; sum them up when the aggregate is absent.
        let count = a.num_items.unwrap_or_else(|| {
            a.num_items_watching.unwrap_or(0)
                + a.num_items_completed.unwrap_or(0)
                + a.num_items_on_hold.unwrap_or(0)
                + a.num_items_dropped.unwrap_or(0)
                + a.num_items_plan_to_watch.unwrap_or(0)
        });
        crate::account::provider::UserStats {
            anime_count: count,
            mean_score_0_to_10: a.mean_score.filter(|s| *s > 0.0),
        }
    });
    Ok(UserProfile {
        provider: ProviderKind::MyAnimeList,
        user_id: wire.id.to_string(),
        username: wire.name,
        avatar_url: wire.picture,
        stats,
    })
}

struct MalListPage {
    entries: Vec<ListEntry>,
    next_url: Option<String>,
}

fn parse_list_page(body: &[u8]) -> Result<MalListPage> {
    #[derive(Deserialize)]
    struct Wire {
        data: Vec<WireRow>,
        #[serde(default)]
        paging: Option<Paging>,
    }
    #[derive(Deserialize)]
    struct Paging {
        #[serde(default)]
        next: Option<String>,
    }
    #[derive(Deserialize)]
    struct WireRow {
        node: WireNode,
        list_status: WireListStatus,
    }
    #[derive(Deserialize)]
    struct WireNode {
        id: u32,
        #[serde(default)]
        title: String,
    }
    #[derive(Deserialize)]
    struct WireListStatus {
        status: String,
        #[serde(default)]
        score: Option<u8>,
        #[serde(default)]
        num_episodes_watched: Option<u32>,
        #[serde(default)]
        is_rewatching: Option<bool>,
        #[serde(default)]
        updated_at: Option<String>,
    }
    let wire: Wire = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("mal list_all page: {e}"),
    })?;
    let mut entries = Vec::with_capacity(wire.data.len());
    for row in wire.data {
        let Some(status) = ListStatus::from_mal(
            &row.list_status.status,
            row.list_status.is_rewatching.unwrap_or(false),
        ) else {
            // Unknown status — log + skip rather than fail the whole
            // page (mirrors AniList's tolerance for unrecognised
            // enum values).
            continue;
        };
        let updated_at_epoch_s = row
            .list_status
            .updated_at
            .as_deref()
            .map_or(0, parse_iso8601_to_epoch);
        // MAL scores are 0..=10 integer; the cache stores 0..=100.
        // 0 means "unrated" — drop it so the popover doesn't render
        // "0/10" for users who haven't scored anything.
        let score_0_to_100 = row
            .list_status
            .score
            .filter(|s| *s > 0)
            .map(|s| s.saturating_mul(10));
        entries.push(ListEntry {
            provider: ProviderKind::MyAnimeList,
            media_id: ProviderMediaId(row.node.id),
            mal_id: Some(row.node.id),
            status,
            progress_episodes: row.list_status.num_episodes_watched.unwrap_or(0),
            score_0_to_100,
            updated_at_epoch_s,
            title: row.node.title,
        });
    }
    Ok(MalListPage {
        entries,
        next_url: wire.paging.and_then(|p| p.next),
    })
}

/// Minimal RFC 3339 / ISO 8601 parser. MAL always emits the canonical
/// `YYYY-MM-DDTHH:MM:SS±HH:MM` (or trailing `Z`) shape — we extract
/// the date + time numerically and ignore the trailing offset (the
/// epoch the cache stores is treated as UTC; ordering across rows
/// stays correct because every row is from the same user).
///
/// Returns 0 for unparseable input so a malformed row doesn't fail
/// the whole list page.
fn parse_iso8601_to_epoch(s: &str) -> i64 {
    let bytes = s.as_bytes();
    if bytes.len() < 19 {
        return 0;
    }
    let parse_u = |start: usize, end: usize| -> Option<i64> {
        std::str::from_utf8(&bytes[start..end]).ok()?.parse().ok()
    };
    let Some(y) = parse_u(0, 4) else { return 0 };
    let Some(m) = parse_u(5, 7) else { return 0 };
    let Some(d) = parse_u(8, 10) else { return 0 };
    let Some(hh) = parse_u(11, 13) else { return 0 };
    let Some(mm) = parse_u(14, 16) else { return 0 };
    let Some(ss) = parse_u(17, 19) else { return 0 };
    // Howard Hinnant's days_from_civil algorithm — exact for all
    // years in the proleptic Gregorian calendar.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m_adj = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * m_adj + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    days * 86400 + hh * 3600 + mm * 60 + ss
}

#[cfg(test)]
#[path = "mal_user_test.rs"]
mod tests;
