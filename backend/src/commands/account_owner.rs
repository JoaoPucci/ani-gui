//! Which tracker user a bearer belongs to, asked once, and whose cached
//! list a tracker write updates.
//!
//! Every cache write the backend makes for a tracker write — the
//! write-through after a save, a removal, a watched episode — and every
//! cached read the Watch Later row makes files its rows under the user
//! the tracker names for the bearer, and each asked the tracker anew:
//! an AniList `Viewer` query spent on every write and every row load. A
//! bearer's user never changes, so the first answer is kept for the
//! bearer and the rest are answered from it. A refused lookup is kept
//! nowhere, and a disconnect forgets the tracker's bearers.
//!
//! What a profile says besides its user — the name, the avatar, the
//! stats the account page shows — is not kept: the `/me` route asks.
//!
//! The cache is kept per user, and only the tracker can say which user
//! a bearer belongs to. A write asks before it goes out: the write may
//! spend the last of the tracker's allowance, and a lookup after it
//! could then be refused, leaving the row it changed stale. A bearer
//! already known is answered from the memo and asks nothing. A rate
//! limit on the lookup holds the write — it goes again once the tracker
//! allows. Any other failure does not stop the write; the lookup is
//! asked once more after it, and with still no answer the row is
//! dropped from every user's cached list rather than left as it was.
//! A list refresh started after the write fills it in again; every write
//! leaves Home's list due for one (`watch-later-refresh.ts`), and a
//! refresh already out when the write settled does not clear that.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::account::provider::{ProviderKind, Tokens, UserListProvider};
use crate::app::AppState;
use crate::error::{AniError, Result};

/// The user each bearer seen so far belongs to, per tracker.
#[derive(Clone, Default)]
pub(crate) struct OwnerMemo(Arc<Mutex<Owners>>);

/// The memo's contents: each bearer's user, and how many times each
/// tracker has been forgotten, so a lookup that was out across a
/// disconnect keeps nothing.
#[derive(Default)]
struct Owners {
    users: HashMap<(ProviderKind, String), String>,
    forgotten: HashMap<ProviderKind, u64>,
}

impl Owners {
    fn generation(&self, kind: ProviderKind) -> u64 {
        self.forgotten.get(&kind).copied().unwrap_or(0)
    }
}

impl OwnerMemo {
    fn owners(&self) -> std::sync::MutexGuard<'_, Owners> {
        self.0.lock().expect("owner memo poisoned")
    }

    /// Forget every bearer of `kind`: its account is gone. A lookup
    /// already out keeps nothing when it answers.
    pub(crate) fn forget(&self, kind: ProviderKind) {
        let mut owners = self.owners();
        owners.users.retain(|(k, _), _| *k != kind);
        *owners.forgotten.entry(kind).or_insert(0) += 1;
    }

    /// The user `tokens` belongs to on `provider`: remembered, or asked
    /// once and remembered — unless the tracker was forgotten while the
    /// lookup was out.
    pub(crate) async fn owner(
        &self,
        kind: ProviderKind,
        provider: &dyn UserListProvider,
        tokens: &Tokens,
    ) -> Result<String> {
        let key = (kind, tokens.access_token.clone());
        let generation = {
            let owners = self.owners();
            if let Some(user) = owners.users.get(&key) {
                return Ok(user.clone());
            }
            owners.generation(kind)
        };
        let user = provider.me(tokens).await?.user_id;
        let mut owners = self.owners();
        if owners.generation(kind) == generation {
            owners.users.insert(key, user.clone());
        }
        Ok(user)
    }
}

/// The owner, asked before a write goes out (answered from the memo for
/// a known bearer). `Err` only for a rate limit, which holds the write;
/// `None` for any other failure.
pub(crate) async fn before_write(
    state: &AppState,
    kind: ProviderKind,
    provider: &dyn UserListProvider,
    tokens: &Tokens,
) -> Result<Option<String>> {
    let owners = &state.account_write_locks.owners;
    match owners.owner(kind, provider, tokens).await {
        Ok(owner) => Ok(Some(owner)),
        Err(e @ AniError::RateLimited { .. }) => Err(e),
        Err(_) => Ok(None),
    }
}

/// The owner after the write: the one asked before, or asked again.
pub(crate) async fn after_write(
    state: &AppState,
    kind: ProviderKind,
    provider: &dyn UserListProvider,
    tokens: &Tokens,
    before: Option<String>,
) -> Option<String> {
    match before {
        Some(owner) => Some(owner),
        None => {
            let owners = &state.account_write_locks.owners;
            owners.owner(kind, provider, tokens).await.ok()
        }
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

#[cfg(test)]
#[path = "account_owner_test.rs"]
mod tests;
