//! The launch refresh: decide the flags within [`SETTLE_CAP`], and
//! keep trying to cache the file for the next launch.
//!
//! The cap is how long the launch waits, not how long the fetch tries.
//!
//! 1. The fresh cached copy is read, if there is one.
//! 2. The fetch starts. If the file arrives within the cap, it decides
//!    the flags. Otherwise the cached copy decides them, or the
//!    defaults when there is none.
//! 3. The fetch goes on in the background, within its own generous
//!    timeout. When the file arrives it is cached for the next launch.
//! 4. When that attempt fails, the file is fetched again later in the
//!    session, after [`retry_delay`], until one attempt succeeds and
//!    is cached.
//!
//! Nothing after step 2 changes this session's flags: the store
//! refuses a second decision. So a connection that always misses the
//! cap costs one launch's delay — the switch the maintainer flipped
//! applies from the next launch — and is never stuck unless the file
//! cannot be downloaded at any point in a whole session.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use super::cache::{self, CacheEntry, FRESH_FOR};
use super::resolve::Document;
use super::store::{FlagStore, Origin, SETTLE_CAP};

/// The waits before the first few retries in a session.
pub const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(3 * 60),
    Duration::from_secs(10 * 60),
    Duration::from_secs(30 * 60),
];

/// The wait between retries after those.
pub const RETRY_EVERY: Duration = Duration::from_secs(60 * 60);

/// How long to wait before retry number `attempt` (from 0).
#[must_use]
pub fn retry_delay(attempt: u32) -> Duration {
    let _ = attempt;
    Duration::ZERO
}

/// Run the launch refresh. `fetch` makes one attempt at the file;
/// `clock` gives seconds since the Unix epoch. With no `cache_path`
/// there is nowhere to keep the file for the next launch, so the
/// refresh ends once the flags are decided and the first attempt is
/// over.
pub async fn refresh<F, Fut, C>(
    store: &FlagStore,
    cache_path: Option<&Path>,
    mut fetch: F,
    clock: C,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<Document>>,
    C: Fn() -> u64,
{
    let _ = (store, cache_path, &mut fetch, clock, decide);
}

async fn read_fresh(path: Option<&Path>, now: u64) -> Option<CacheEntry> {
    cache::read(path?)
        .await
        .filter(|e| cache::is_fresh(e.fetched_at, now, FRESH_FOR))
}

/// Settle the store from the file when it arrived in time, else from
/// the cache, else at the defaults.
fn decide(store: &FlagStore, fetched: Option<&Document>, cached: Option<&CacheEntry>) {
    let (doc, origin) = match (fetched, cached) {
        (Some(doc), _) => (Some(doc), Origin::File),
        (None, Some(entry)) => (Some(&entry.flags), Origin::Cache),
        (None, None) => (None, Origin::Defaults),
    };
    store.settle(doc, origin);
    tracing::info!(
        origin = store.origin().as_str(),
        flags = ?store.current(),
        "remote flags settled"
    );
}

#[cfg(test)]
#[path = "refresh_test.rs"]
mod tests;

#[cfg(test)]
#[path = "refresh_prop_test.rs"]
mod prop_tests;
