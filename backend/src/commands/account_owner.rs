//! Whose cached list a tracker write updates.
//!
//! The cache is kept per user, and only the tracker can say which user
//! a bearer belongs to. A write asks before it goes out: the write may
//! spend the last of the tracker's allowance, and a lookup after it
//! could then be refused, leaving the row it changed stale. A rate limit
//! on the lookup holds the write — it goes again once the tracker
//! allows. Any other failure does not stop the write; the lookup is
//! asked once more after it, and with still no answer the row is
//! dropped from every user's cached list rather than left as it was,
//! for the next list refresh to fill in.

use std::sync::Arc;

use crate::account::provider::{ProviderKind, Tokens, UserListProvider};
use crate::app::AppState;
use crate::error::{AniError, Result};

/// The owner, asked before a write goes out. `Err` only for a rate
/// limit, which holds the write; `None` for any other failure.
pub(crate) async fn before_write(
    provider: &dyn UserListProvider,
    tokens: &Tokens,
) -> Result<Option<String>> {
    match provider.me(tokens).await {
        Ok(profile) => Ok(Some(profile.user_id)),
        Err(e @ AniError::RateLimited { .. }) => Err(e),
        Err(_) => Ok(None),
    }
}

/// The owner after the write: the one asked before, or asked again.
pub(crate) async fn after_write(
    provider: &dyn UserListProvider,
    tokens: &Tokens,
    before: Option<String>,
) -> Option<String> {
    match before {
        Some(owner) => Some(owner),
        None => provider.me(tokens).await.ok().map(|p| p.user_id),
    }
}

/// Update the cached row of `media` that a write in connection epoch
/// `began` changed: `write` it for `owner`, or with no owner known drop
/// the row for every user. Marked as written either way, so a refresh
/// already out cannot put the old row back. Best-effort, and nothing
/// for an account disconnected since.
pub(crate) fn cache_row(
    state: &Arc<AppState>,
    kind: ProviderKind,
    began: u64,
    media: u32,
    owner: Option<&str>,
    write: impl FnOnce(&str) -> Result<()>,
) {
    let locks = &state.account_write_locks;
    let _ = locks.epochs.write_if_current(kind, began, || {
        locks
            .cache_revisions
            .write_row(kind, media, || match owner {
                Some(owner) => write(owner),
                None => {
                    crate::account::cache_upsert::delete_media_rows(&state.cache_pool, kind, media)
                }
            })
    });
}
