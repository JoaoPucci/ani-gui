//! Which connection of a tracker account a cache write belongs to.
//!
//! Disconnecting a tracker purges that account's cached list. A write
//! or list read already out at that moment answers afterwards, and its
//! write-through would put the row — or the whole list — back for the
//! account just disconnected. Each purge starts a new epoch for the
//! provider; a write-through names the epoch its request began in and
//! lands only if no purge came since. The check and the write happen
//! under one lock, and so do the purge and the bump, so a purge cannot
//! fall between a write's check and its write.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::account::provider::ProviderKind;

/// A provider's connection epochs, process-wide.
#[derive(Clone, Default)]
pub struct AccountEpochs {
    inner: Arc<Mutex<HashMap<ProviderKind, u64>>>,
}

impl AccountEpochs {
    fn epochs(&self) -> std::sync::MutexGuard<'_, HashMap<ProviderKind, u64>> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The epoch a request for `kind` begins in.
    pub(crate) fn current(&self, kind: ProviderKind) -> u64 {
        self.epochs().get(&kind).copied().unwrap_or(0)
    }

    /// Run `write` only if no purge of `kind` came since `began`.
    pub(crate) fn write_if_current<T>(
        &self,
        kind: ProviderKind,
        began: u64,
        write: impl FnOnce() -> T,
    ) -> Option<T> {
        let epochs = self.epochs();
        (epochs.get(&kind).copied().unwrap_or(0) == began).then(write)
    }

    /// Purge `kind`'s cache with `purge`, starting a new epoch.
    pub(crate) fn purge<T>(&self, kind: ProviderKind, purge: impl FnOnce() -> T) -> T {
        let mut epochs = self.epochs();
        *epochs.entry(kind).or_insert(0) += 1;
        purge()
    }
}
