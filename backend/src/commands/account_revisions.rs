//! Which cached rows a write changed while a full-list read was out.
//!
//! A refresh reads the whole list and replaces the cached one with it.
//! A write that lands while that read is out has cached a newer answer
//! for its row than the read can carry, so the refresh must leave that
//! row alone — and only that row: the rest of its answer is the newest
//! the app has for them. Each write marks its row with the next
//! revision; a refresh notes the revision it started at and, when its
//! answer arrives, writes every row except those marked since.
//!
//! A refresh also takes a revision of its own when it starts, so two
//! refreshes are ordered: one that started earlier and arrives after a
//! later one has already been cached carries nothing newer than it —
//! the rows written since the later one started are kept either way —
//! and is not cached at all.
//!
//! A live read of one entry takes a revision as well. When it finds the
//! entry absent, a removal can clear the stale cached row on that
//! evidence — but only while no newer answer reached the cache: no write
//! marked the row and no refresh started after the read was cached. The
//! cleared row is marked as of the read, so a refresh that started
//! before the read cannot put it back, and one started after it can.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::account::provider::ProviderKind;

#[derive(Default)]
struct Revisions {
    /// The latest revision per provider.
    latest: HashMap<ProviderKind, u64>,
    /// The revision each written row was last marked with.
    rows: HashMap<(ProviderKind, u32), u64>,
    /// The start of the latest refresh cached, per provider.
    cached_from: HashMap<ProviderKind, u64>,
    /// Rows whose last live read found them absent: the revision that
    /// read started at.
    absent: HashMap<(ProviderKind, u32), u64>,
}

impl Revisions {
    fn next(&mut self, kind: ProviderKind) -> u64 {
        let latest = self.latest.entry(kind).or_insert(0);
        *latest += 1;
        *latest
    }
}

/// The rows of `kind` marked after `since`.
fn written_since(
    rows: &HashMap<(ProviderKind, u32), u64>,
    kind: ProviderKind,
    since: u64,
) -> HashSet<u32> {
    rows.iter()
        .filter(|((k, _), &rev)| *k == kind && rev > since)
        .map(|((_, id), _)| *id)
        .collect()
}

/// Cache revisions, process-wide.
#[derive(Clone, Default)]
pub struct CacheRevisions {
    inner: Arc<Mutex<Revisions>>,
}

impl CacheRevisions {
    fn lock(&self) -> std::sync::MutexGuard<'_, Revisions> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A refresh of `kind` starts: the revision it reads from.
    pub(crate) fn start_refresh(&self, kind: ProviderKind) -> u64 {
        self.lock().next(kind)
    }

    /// A live read of an entry of `kind` starts: the revision it reads at.
    pub(crate) fn start_read(&self, kind: ProviderKind) -> u64 {
        self.lock().next(kind)
    }

    /// What the live read of `media` that started at `read` found.
    pub(crate) fn note_read(&self, kind: ProviderKind, media: u32, read: u64, absent: bool) {
        let mut revisions = self.lock();
        if absent {
            revisions.absent.insert((kind, media), read);
        } else {
            revisions.absent.remove(&(kind, media));
        }
    }

    /// Clear `media`'s cached row with `clear` on the evidence of its last
    /// live read, if that read found it absent and no write or cached
    /// refresh has answered for the row since. `None` — nothing cleared —
    /// otherwise. The evidence is used up either way.
    pub(crate) fn clear_if_still_absent<T>(
        &self,
        kind: ProviderKind,
        media: u32,
        clear: impl FnOnce() -> T,
    ) -> Option<T> {
        let mut revisions = self.lock();
        let read = revisions.absent.remove(&(kind, media))?;
        // A refresh started after the read and cached since has withdrawn
        // the evidence already (`cache_refresh`); a write since marks the row.
        if revisions
            .rows
            .get(&(kind, media))
            .is_some_and(|&rev| rev > read)
        {
            return None;
        }
        revisions.rows.insert((kind, media), read);
        Some(clear())
    }

    /// Mark `media` of `kind` written and apply `change` to its cached
    /// row under the same lock, so no refresh can check the marks
    /// between the two.
    pub(crate) fn write_row<T>(
        &self,
        kind: ProviderKind,
        media: u32,
        change: impl FnOnce() -> T,
    ) -> T {
        let mut revisions = self.lock();
        let rev = revisions.next(kind);
        revisions.rows.insert((kind, media), rev);
        change()
    }

    /// Cache the answer of the refresh that started at `since` with
    /// `write`, handed the rows written since, which it must leave as
    /// they are. `None` — nothing written — when a refresh that started
    /// later is already cached.
    pub(crate) fn cache_refresh<T, E>(
        &self,
        kind: ProviderKind,
        since: u64,
        write: impl FnOnce(&HashSet<u32>) -> Result<T, E>,
    ) -> Option<Result<T, E>> {
        let mut revisions = self.lock();
        if revisions.cached_from.get(&kind).is_some_and(|&c| c > since) {
            return None;
        }
        let keep = written_since(&revisions.rows, kind, since);
        let written = write(&keep);
        if written.is_ok() {
            revisions.cached_from.insert(kind, since);
            // A refresh started before this one is never cached now, and
            // one started after it reads only marks newer than its start.
            revisions
                .rows
                .retain(|(k, _), rev| *k != kind || *rev > since);
            // A read before this refresh can no longer justify a clear:
            // the refresh's answer for the row is newer.
            revisions
                .absent
                .retain(|(k, _), read| *k != kind || *read > since);
        }
        Some(written)
    }
}

#[cfg(test)]
#[path = "account_revisions_prop_test.rs"]
mod prop_tests;
