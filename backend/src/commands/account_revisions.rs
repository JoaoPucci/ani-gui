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
//! A live read of one entry takes a revision as well, when it is sent.
//! When it finds the entry absent, a removal can clear the stale cached
//! row on that evidence — but only while no answer asked after it has
//! reached the cache: no write marked the row since the read was sent,
//! no refresh started after it was cached, and no read sent after it
//! found the entry listed. Reads and refreshes are ordered by when they
//! were asked, not when they arrived, since answers arrive in any order;
//! a write is marked when it is cached, which is no earlier than when it
//! was sent, so it can only stop a clear more often, never less. The cleared
//! row is marked as of the read, so a refresh that started before the
//! read cannot put it back, and one started after it can.

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
    /// Each row's latest-sent live read that has answered: the revision
    /// it started at, and whether it found the row absent (and has not
    /// been used for a clear yet).
    reads: HashMap<(ProviderKind, u32), (u64, bool)>,
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

    /// What the live read of `media` that started at `read` found. A read
    /// sent before one already noted says nothing newer, and is dropped.
    pub(crate) fn note_read(&self, kind: ProviderKind, media: u32, read: u64, absent: bool) {
        let mut revisions = self.lock();
        let noted = revisions
            .reads
            .entry((kind, media))
            .or_insert((read, absent));
        if noted.0 <= read {
            *noted = (read, absent);
        }
    }

    /// Clear `media`'s cached row with `clear` on the evidence of its
    /// latest-sent live read, if that read found it absent and nothing
    /// asked after it has answered for the row: no write since, and no
    /// refresh started after it and cached. `None` — nothing cleared —
    /// otherwise. Evidence clears once.
    pub(crate) fn clear_if_still_absent<T>(
        &self,
        kind: ProviderKind,
        media: u32,
        clear: impl FnOnce() -> T,
    ) -> Option<T> {
        let mut revisions = self.lock();
        let &(read, absent) = revisions.reads.get(&(kind, media))?;
        let refreshed = revisions.cached_from.get(&kind).is_some_and(|&c| c > read);
        let written = revisions
            .rows
            .get(&(kind, media))
            .is_some_and(|&rev| rev > read);
        if !absent || refreshed || written {
            return None;
        }
        revisions.reads.insert((kind, media), (read, false));
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
                .reads
                .retain(|(k, _), (read, _)| *k != kind || *read > since);
        }
        Some(written)
    }
}

#[cfg(test)]
#[path = "account_revisions_prop_test.rs"]
mod prop_tests;
