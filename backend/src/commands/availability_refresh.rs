//! Write ordering for the availability cache.
//!
//! Two lookups for the same row can be in flight at once: the one a
//! page fires on load, and the cache-bypassing one a user's click
//! sends when they re-ask about a dimmed episode. `meta_cache_put` is
//! INSERT OR REPLACE, so without an ordering rule the request that
//! finishes last owns the row — regardless of which question it
//! answered.
//!
//! That default is wrong here. The ordinary lookup read THROUGH the
//! cache to get its answer, so when it lands second it reinstates the
//! very count the refresh was sent to replace, and holds it for the
//! row's whole TTL — 24 hours on an ongoing show, 30 days on a
//! finished one. The next visit then re-gates the episode the user
//! had just unlocked.
//!
//! A timestamp cannot stand in for this: `meta_cache.fetched_at` is
//! whole seconds and the race is sub-second.
//!
//! The same map counts the positive rows written for a key, for a
//! second question the generation cannot answer: whether the positive
//! row standing when a negative comes to be written is the one its
//! walk set out from. A resolve's or a probe's success is not a
//! cache-bypassing refresh and leaves the generation where it was,
//! and a success that proves the show again through the same provider
//! writes the row back byte for byte — so neither the generation nor
//! the row's bytes can tell a late miss that such a success landed
//! while it was out. The count of positive writes can.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[path = "availability_refresh_rules.rs"]
mod rules;
pub use rules::{generation_at_start, hold_if_still_ours, may_write_cache, with_row_if_ours};

/// The lock held across one row's read-decide-write section. Async
/// because the section spans an await: a negative answer for a
/// pre-premiere show fetches the airing schedule before it writes.
pub(crate) type RowLock = Arc<tokio::sync::Mutex<()>>;

/// The counts kept per row: writes made by a cache-bypassing
/// refresh, positive rows written by anything at all, and replays
/// served from a standing positive row without writing it.
#[derive(Clone, Copy, Default)]
struct RowCounts {
    refreshes: u64,
    positives: u64,
    replays: u64,
}

/// Per-`(kitsu_id, mode)` count of cache writes made by a
/// cache-bypassing refresh, of positive rows written, and the lock
/// that makes reading those counts mean something.
///
/// Keyed per show AND mode because the provider catalogues sub and dub
/// separately — a refresh of the dub row says nothing about whether
/// the sub row is current.
///
/// Process-wide, like the other write-ordering state on `AppState`;
/// the cloned `Arc` is cheap.
#[derive(Clone, Default)]
pub struct AvailabilityRefreshes {
    inner: Arc<Mutex<HashMap<String, RowCounts>>>,
    locks: Arc<Mutex<HashMap<String, RowLock>>>,
}

impl AvailabilityRefreshes {
    /// An empty map. Production builds one at boot; tests per fixture.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many refreshes have written this row. Captured by a lookup
    /// before it goes out, and read again before it writes.
    #[must_use]
    pub fn generation(&self, key: &str) -> u64 {
        self.inner
            .lock()
            .map(|m| m.get(key).map_or(0, |c| c.refreshes))
            .unwrap_or(0)
    }

    /// Record that a refresh is writing this row.
    pub fn bump(&self, key: &str) {
        if let Ok(mut m) = self.inner.lock() {
            m.entry(key.to_string()).or_default().refreshes += 1;
        }
    }

    /// How many positive rows this process has written for the key —
    /// a resolve's success, a probe's, a replay's count-less row.
    /// Captured by a writer before it goes out and read again under
    /// the lock, so a stamp can tell a positive row written while it
    /// was out, even one that put the same bytes back, from the row
    /// it set out from.
    #[must_use]
    pub fn positives(&self, key: &str) -> u64 {
        self.inner
            .lock()
            .map(|m| m.get(key).map_or(0, |c| c.positives))
            .unwrap_or(0)
    }

    /// Record that a positive row was written for this key.
    pub fn note_positive(&self, key: &str) {
        if let Ok(mut m) = self.inner.lock() {
            m.entry(key.to_string()).or_default().positives += 1;
        }
    }

    /// How many replays have been served from a standing positive
    /// row of this key without writing it. Proof a stream played,
    /// which a miss out at the time never weighed — but not a row
    /// written, so a later success is still free to write its own.
    #[must_use]
    pub fn replays(&self, key: &str) -> u64 {
        self.inner
            .lock()
            .map(|m| m.get(key).map_or(0, |c| c.replays))
            .unwrap_or(0)
    }

    /// Record that a replay was served from this key's standing row.
    pub fn note_replay(&self, key: &str) {
        if let Ok(mut m) = self.inner.lock() {
            m.entry(key.to_string()).or_default().replays += 1;
        }
    }

    /// The lock guarding one row's whole decide-and-write section.
    ///
    /// The generation counter on its own only narrows the race it was
    /// meant to close: a lookup can read the counter, find it
    /// unchanged, and still be overtaken before it writes — the
    /// section is not a single step, and it yields in the middle of
    /// itself. Whoever resumes last then owns the row, which is the
    /// behaviour the counter exists to prevent.
    ///
    /// Taken before the check and held past the write, so the reading
    /// still describes the row at the moment of writing. Scoped per
    /// row, so probes for different shows — or for the other
    /// catalogue of the same show — never wait on each other. The
    /// brief std-mutex section only clones the `Arc`; the async mutex
    /// it hands back is what the caller holds across the await.
    pub(crate) fn for_row(&self, key: &str) -> RowLock {
        let mut map = self
            .locks
            .lock()
            .expect("availability row-lock map poisoned");
        Arc::clone(map.entry(key.to_owned()).or_default())
    }
}

#[cfg(test)]
#[path = "availability_refresh_test.rs"]
mod tests;
