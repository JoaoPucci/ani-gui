//! Which tracker user a bearer belongs to, asked once.
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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::account::provider::{ProviderKind, Tokens, UserListProvider};
use crate::error::Result;

/// The user each bearer seen so far belongs to, per tracker.
#[derive(Clone, Default)]
pub(crate) struct OwnerMemo(Arc<Mutex<HashMap<(ProviderKind, String), String>>>);

impl OwnerMemo {
    fn owners(&self) -> std::sync::MutexGuard<'_, HashMap<(ProviderKind, String), String>> {
        self.0.lock().expect("owner memo poisoned")
    }

    /// Forget every bearer of `kind`: its account is gone.
    pub(crate) fn forget(&self, kind: ProviderKind) {
        self.owners().retain(|(k, _), _| *k != kind);
    }

    /// The user `tokens` belongs to on `provider`: remembered, or asked
    /// once and remembered.
    pub(crate) async fn owner(
        &self,
        kind: ProviderKind,
        provider: &dyn UserListProvider,
        tokens: &Tokens,
    ) -> Result<String> {
        let key = (kind, tokens.access_token.clone());
        if let Some(user) = self.owners().get(&key) {
            return Ok(user.clone());
        }
        let user = provider.me(tokens).await?.user_id;
        self.owners().insert(key, user.clone());
        Ok(user)
    }
}

#[cfg(test)]
#[path = "account_owner_test.rs"]
mod tests;
