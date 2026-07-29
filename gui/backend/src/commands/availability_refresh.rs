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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// The lock held across one row's read-decide-write section. Async
/// because the section spans an await: a negative answer for a
/// pre-premiere show fetches the airing schedule before it writes.
pub(crate) type RowLock = Arc<tokio::sync::Mutex<()>>;

/// Per-`(kitsu_id, mode)` count of cache writes made by a
/// cache-bypassing refresh, and the lock that makes reading that
/// count mean something.
///
/// Keyed per show AND mode because allmanga catalogues sub and dub
/// separately — a refresh of the dub row says nothing about whether
/// the sub row is current.
///
/// Process-wide, like the other write-ordering state on `AppState`;
/// the cloned `Arc` is cheap.
#[derive(Clone, Default)]
pub struct AvailabilityRefreshes {
    inner: Arc<Mutex<HashMap<String, u64>>>,
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
            .map(|m| m.get(key).copied().unwrap_or(0))
            .unwrap_or(0)
    }

    /// Record that a refresh is writing this row.
    pub fn bump(&self, key: &str) {
        if let Ok(mut m) = self.inner.lock() {
            *m.entry(key.to_string()).or_insert(0) += 1;
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
        let mut map = self.locks.lock().expect("availability row-lock map poisoned");
        Arc::clone(map.entry(key.to_owned()).or_default())
    }
}

/// The refresh count for the row a lookup is about to ask about,
/// captured before any network work so it can be compared again at
/// write time.
///
/// Zero when there is no Kitsu id: nothing is cached under one, so
/// there is no row to lose a race over.
#[must_use]
pub fn generation_at_start(
    refreshes: &AvailabilityRefreshes,
    kitsu_id: Option<&str>,
    mode: &str,
) -> u64 {
    kitsu_id.filter(|s| !s.is_empty()).map_or(0, |id| {
        refreshes.generation(&super::availability::cache_key(id, mode))
    })
}

/// Whether a lookup may still write the row it was asked about.
///
/// A refresh always may: it skipped the cache, so it is not the stale
/// one even when another refresh beat it, and between two of those
/// last-write-wins is the right rule. An ordinary lookup may only if
/// no refresh has written since it went out.
#[must_use]
pub fn may_write_cache(
    refreshes: &AvailabilityRefreshes,
    key: &str,
    generation_at_start: u64,
    bypass_cache: bool,
) -> bool {
    bypass_cache || refreshes.generation(key) == generation_at_start
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::availability::cache_key;

    #[test]
    fn an_ordinary_lookup_yields_to_a_refresh_that_landed_while_it_was_out() {
        let refreshes = AvailabilityRefreshes::new();
        let key = cache_key("kid-1", "sub");
        let started_at = refreshes.generation(&key);

        // The user clicked a dimmed tile and the cache-bypassing
        // lookup came back first, writing the fresh row.
        refreshes.bump(&key);

        // This one has been out since before that. Its answer is
        // older, and it read through the cache to get it — writing it
        // now puts the stale count back for the whole TTL, so the next
        // page visit restores the gate the refresh just cleared.
        assert!(!may_write_cache(&refreshes, &key, started_at, false));
    }

    #[test]
    fn a_refresh_writes_even_when_another_refresh_landed_first() {
        let refreshes = AvailabilityRefreshes::new();
        let key = cache_key("kid-1", "sub");
        let started_at = refreshes.generation(&key);
        refreshes.bump(&key);

        // Both skipped the cache, so neither is the stale one — last
        // write wins is the right rule between them.
        assert!(may_write_cache(&refreshes, &key, started_at, true));
    }

    #[test]
    fn an_undisturbed_lookup_still_writes() {
        let refreshes = AvailabilityRefreshes::new();
        let key = cache_key("kid-1", "sub");
        let started_at = refreshes.generation(&key);

        assert!(may_write_cache(&refreshes, &key, started_at, false));
    }

    #[test]
    fn generations_do_not_leak_between_shows_or_modes() {
        let refreshes = AvailabilityRefreshes::new();
        let sub = cache_key("kid-1", "sub");
        let dub = cache_key("kid-1", "dub");
        let other = cache_key("kid-2", "sub");
        let sub_started = refreshes.generation(&sub);

        refreshes.bump(&dub);
        refreshes.bump(&other);

        // A refresh for the dub catalogue, or for a different show,
        // says nothing about this row.
        assert!(may_write_cache(&refreshes, &sub, sub_started, false));
    }
}
