//! Network plumbing for [`super::mal_user::MalProvider`]. Holds the
//! shared `post_token_form` + `get_auth_bytes` helpers, the
//! refresh-coalesce cache type, and the inner refresh implementation
//! (mutex acquisition + cache hit/miss + network rotation). Extracted
//! so the trait-impl file in `mal_user.rs` stays narrow enough to
//! clear the CRAP ratchet.
//!
//! Everything here is `pub(super)` — the trait impl is the only
//! caller. The helpers expect a parent reference (`MalProvider`) and
//! delegate field access through accessors `mal_user.rs` exposes.

use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;

use super::mal_user::MalProvider;
use super::mal_user_parse::parse_token_response;
use crate::account::credentials::MAL_CLIENT_ID;
use crate::account::provider::Tokens;
use crate::error::{AniError, Result};

/// `User-Agent` advertised on every MAL request. Per the API license
/// notes (Phase 0), we identify clearly so MAL can correlate traffic
/// if they ever audit.
pub(super) const MAL_USER_AGENT: &str = concat!("ani-gui/", env!("CARGO_PKG_VERSION"));

/// Cache slot for the last successful refresh, keyed by the input
/// refresh token. Lets concurrent refreshers share one upstream
/// rotation instead of each invalidating the previous result.
pub(super) struct CoalescedRefresh {
    pub input_refresh_token: String,
    pub tokens: Tokens,
}

impl MalProvider {
    /// Shared form-encoded POST to MAL's OAuth token endpoint. Both
    /// `exchange_code` and `refresh` use it — only the form body
    /// differs. Returns parsed `Tokens` on 2xx, `AniError::Upstream`
    /// for non-2xx, `AniError::Network` for transport failures.
    pub(super) async fn post_token_form(&self, form: &[(&str, &str)]) -> Result<Tokens> {
        let resp = self
            .client()
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
    /// `X-MAL-CLIENT-ID` header. Used by `me` and `list_all` (and any
    /// future read endpoint).
    pub(super) async fn get_auth_bytes(&self, url: &str, tokens: &Tokens) -> Result<Bytes> {
        let resp = self
            .client()
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

    /// Inner `refresh` implementation. Holds the mutex across the
    /// cache-check + network call so two concurrent refreshers
    /// serialize, hits the cache when the input refresh token
    /// matches a previously-rotated set and that set hasn't yet
    /// expired (Codex P2 #3375578767), otherwise rotates and stores
    /// the result.
    pub(super) async fn refresh_inner(&self, refresh_token: &str) -> Result<Tokens> {
        let mut guard = self.last_refresh().lock().await;
        if let Some(cached) = guard.as_ref() {
            if cached.input_refresh_token == refresh_token {
                let now_s = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
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
}

/// Extract the (scheme, host, port) tuple of a URL string for origin
/// comparison. Returns `("", "", 0)` for unparseable input — the
/// caller treats that as a non-matching origin so a malformed
/// `paging.next` value is dropped rather than followed (Codex P2
/// #3375623170).
pub(super) fn url_origin(s: &str) -> (String, String, u16) {
    let Ok(u) = url::Url::parse(s) else {
        return (String::new(), String::new(), 0);
    };
    let host = u.host_str().unwrap_or("").to_string();
    let port = u
        .port_or_known_default()
        .or_else(|| {
            if u.scheme() == "http" {
                Some(80)
            } else {
                Some(443)
            }
        })
        .unwrap_or(0);
    (u.scheme().to_string(), host, port)
}
