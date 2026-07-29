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
        let mut map = self
            .locks
            .lock()
            .expect("availability row-lock map poisoned");
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

/// Take the row and report whether this writer may still have it.
///
/// `Some(guard)` — write now, and hold the guard until the write has
/// landed. `None` — a refresh answered while this one was out, and its
/// row is the one that should survive.
///
/// This is the whole protocol in one call, and every writer of an
/// availability row goes through it. The lookup is not the only one:
/// a play resolution stamps the row on success and on a confirmed
/// miss, and it too holds an answer from before it started writing
/// (Codex P2 #3674767151). A writer outside this function is a writer
/// that can put a stale cap back for the row's whole TTL.
pub async fn hold_if_still_ours(
    refreshes: &AvailabilityRefreshes,
    key: &str,
    generation_at_start: u64,
    bypass_cache: bool,
) -> Option<tokio::sync::OwnedMutexGuard<()>> {
    let guard = refreshes.for_row(key).lock_owned().await;
    if !may_write_cache(refreshes, key, generation_at_start, bypass_cache) {
        return None;
    }
    // Inside the lock, so the count a later writer reads already
    // includes this one.
    if bypass_cache {
        refreshes.bump(key);
    }
    Some(guard)
}

/// Run a write under the row, if the row is still this writer's.
///
/// The synchronous counterpart to [`hold_if_still_ours`], and the one
/// to reach for. Taking the write as a closure means the permission
/// cannot be held apart from the thing it permits: a caller that
/// tests the guard and then writes — `hold_if_still_ours(..).await
/// .is_some()` — has already dropped it, and the row is free again
/// for exactly as long as the write takes (Codex P2 #3675142224).
///
/// `Some(_)` with whatever the write returned, or `None` when a
/// refresh answered while this writer was out.
pub async fn with_row_if_ours<T>(
    refreshes: &AvailabilityRefreshes,
    key: &str,
    generation_at_start: u64,
    bypass_cache: bool,
    write: impl FnOnce() -> T,
) -> Option<T> {
    let _writing = hold_if_still_ours(refreshes, key, generation_at_start, bypass_cache).await?;
    Some(write())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::availability::cache_key;

    #[tokio::test]
    async fn a_write_runs_while_the_row_is_still_held() {
        // The guard has to outlive the write, not just precede it.
        // `hold_if_still_ours(..).await.is_some()` reads as a check and
        // is really a drop: the row is free again by the time the write
        // runs, and on the multi-thread runtime production uses, a
        // refresh can take it and land its row in that gap — after
        // which the stale write replaces it for the row's whole TTL.
        //
        // So the write goes inside, where it cannot be separated from
        // the permission to make it.
        let refreshes = AvailabilityRefreshes::new();
        let key = cache_key("kid-w", "sub");
        let started_at = refreshes.generation(&key);

        let held_during_write = with_row_if_ours(&refreshes, &key, started_at, false, || {
            refreshes.for_row(&key).try_lock().is_err()
        })
        .await;

        assert_eq!(
            held_during_write,
            Some(true),
            "the row must still be locked while the write runs"
        );
        // And released afterwards, or the next writer for this show
        // would wait forever.
        assert!(refreshes.for_row(&key).try_lock().is_ok());
    }

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

    proptest::proptest! {
        // The rule `may_write_cache` applies, stated over every shape
        // the two inputs can take rather than the four the examples
        // above pin:
        //
        //   • A refresh writes unconditionally. It skipped the cache,
        //     so no reading it could be holding is the stale one, and
        //     between two refreshes last-write-wins is correct.
        //   • An ordinary lookup writes exactly when no refresh has
        //     landed since it went out — never on a count that moved,
        //     always on one that did not.
        //
        // The predicate decides whether a cached cap outlives its own
        // correction, so "some refresh, some lookup, some interleaving"
        // is the honest domain to state it over.
        #[test]
        fn a_refresh_writes_whatever_happened_while_it_was_out(
            landed in 0usize..8,
        ) {
            let refreshes = AvailabilityRefreshes::new();
            let key = cache_key("kid-p", "sub");
            let started_at = refreshes.generation(&key);
            for _ in 0..landed {
                refreshes.bump(&key);
            }

            proptest::prop_assert!(may_write_cache(&refreshes, &key, started_at, true));
        }

        #[test]
        fn a_lookup_writes_exactly_when_no_refresh_landed(
            landed in 0usize..8,
        ) {
            let refreshes = AvailabilityRefreshes::new();
            let key = cache_key("kid-p", "sub");
            let started_at = refreshes.generation(&key);
            for _ in 0..landed {
                refreshes.bump(&key);
            }

            proptest::prop_assert_eq!(
                may_write_cache(&refreshes, &key, started_at, false),
                landed == 0
            );
        }

        #[test]
        fn only_a_refresh_of_this_very_row_takes_a_lookup_s_turn(
            id in "[a-z0-9]{1,6}",
            other_id in "[a-z0-9]{1,6}",
            noise in 0usize..6,
            on_this_row in proptest::bool::ANY,
        ) {
            // sub and dub are separate catalogues, and separate shows
            // are separate rows, so a refresh of one settles nothing
            // about any other. Whatever else lands, the answer tracks
            // this row and only this row.
            let refreshes = AvailabilityRefreshes::new();
            let key = cache_key(&id, "sub");
            let started_at = refreshes.generation(&key);
            for _ in 0..noise {
                refreshes.bump(&cache_key(&id, "dub"));
                refreshes.bump(&cache_key(&other_id, "dub"));
            }
            if on_this_row {
                refreshes.bump(&key);
            }

            proptest::prop_assert_eq!(
                may_write_cache(&refreshes, &key, started_at, false),
                !on_this_row
            );
        }
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
