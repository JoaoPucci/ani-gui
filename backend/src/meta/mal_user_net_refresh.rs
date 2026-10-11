//! The coalesced refresh and the origin comparison the list pager
//! follows `paging.next` by; split from [`super`] so each file stays
//! inside the CRAP gate's per-file bar.

use super::*;

impl MalProvider {
    /// Inner `refresh` implementation. Holds the mutex across the
    /// cache-check + network call so two concurrent refreshers
    /// serialize, hits the cache when the input refresh token
    /// matches a previously-rotated set and that set hasn't yet
    /// expired (Codex P2 #3375578767), otherwise rotates and stores
    /// the result.
    pub(in crate::meta) async fn refresh_inner(&self, refresh_token: &str) -> Result<Tokens> {
        let mut guard = self.refresh_state().lock().lock().await;
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
pub(in crate::meta) fn url_origin(s: &str) -> (String, String, u16) {
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
