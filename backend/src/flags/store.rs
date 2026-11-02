//! What the rest of the backend reads, and the launch refresh that
//! fills it.
//!
//! The store starts at the built-in defaults, so nothing waits on the
//! network to start. The launch refresh runs as a task of its own: it
//! publishes a fresh cached document as soon as it has read it, then
//! the fetched one (or, failing that, the cache or the defaults
//! again) once the request is done, and marks the store settled.
//! [`FlagStore::current`] never waits; [`FlagStore::settled`] waits
//! for the refresh, but never longer than [`SETTLE_LIMIT`].

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

use super::cache::{self, CacheEntry, FRESH_FOR};
use super::decl::FlagDecl;
use super::remote::{self, FETCH_TIMEOUT, FLAGS_URL};
use super::resolve::{resolve, Document, Resolved};

/// The longest [`FlagStore::settled`] waits for the launch refresh
/// before answering with what the store holds.
pub const SETTLE_LIMIT: Duration = Duration::from_secs(FETCH_TIMEOUT.as_secs() + 2);

#[derive(Debug, Clone)]
struct Snapshot {
    flags: Resolved,
    settled: bool,
}

/// The resolved flags, shared by every handler.
#[derive(Clone)]
pub struct FlagStore {
    decls: &'static [FlagDecl],
    tx: Arc<watch::Sender<Snapshot>>,
}

impl FlagStore {
    /// A store at the defaults, waiting for the launch refresh.
    #[must_use]
    pub fn pending(decls: &'static [FlagDecl]) -> Self {
        Self::with(decls, false)
    }

    /// A store at the defaults that no refresh will change; for a
    /// state that never makes the request.
    #[must_use]
    pub fn fixed(decls: &'static [FlagDecl]) -> Self {
        Self::with(decls, true)
    }

    fn with(decls: &'static [FlagDecl], settled: bool) -> Self {
        let (tx, _rx) = watch::channel(Snapshot {
            flags: resolve(decls, None),
            settled,
        });
        Self {
            decls,
            tx: Arc::new(tx),
        }
    }

    /// The flags as they stand. Never waits.
    #[must_use]
    pub fn current(&self) -> Resolved {
        self.tx.borrow().flags.clone()
    }

    /// Whether the launch refresh has finished.
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.tx.borrow().settled
    }

    /// The flags once the launch refresh has finished, or as they
    /// stand after [`SETTLE_LIMIT`], whichever comes first.
    pub async fn settled(&self) -> Resolved {
        let mut rx = self.tx.subscribe();
        // A timeout leaves the store as it stands; the sender lives as
        // long as `self`, so the wait cannot fail any other way.
        let _ = tokio::time::timeout(SETTLE_LIMIT, rx.wait_for(|s| s.settled)).await;
        self.current()
    }

    fn publish(&self, doc: Option<&Document>, settled: bool) {
        let flags = resolve(self.decls, doc);
        self.tx.send_replace(Snapshot { flags, settled });
    }
}

/// Where the launch refresh reads from and caches to.
#[derive(Debug, Clone)]
pub struct Source {
    /// The flags file's URL.
    pub url: String,
    /// The cache file.
    pub cache_path: PathBuf,
    /// The bound on the whole request.
    pub timeout: Duration,
}

/// Fill `store` for this launch: the fresh cache first, then the
/// fetched file, then settle. `now` is seconds since the Unix epoch.
pub async fn refresh(store: &FlagStore, client: &reqwest::Client, source: &Source, now: u64) {
    let cached = cache::read(&source.cache_path)
        .await
        .filter(|e| cache::is_fresh(e.fetched_at, now, FRESH_FOR));
    if let Some(entry) = &cached {
        store.publish(Some(&entry.flags), false);
    }
    let fetched = remote::fetch(client, &source.url, source.timeout).await;
    let origin = match (&fetched, &cached) {
        (Some(_), _) => "file",
        (None, Some(_)) => "cache",
        (None, None) => "defaults",
    };
    let doc = fetched.as_ref().or(cached.as_ref().map(|e| &e.flags));
    store.publish(doc, true);
    tracing::info!(target: "flags", origin, flags = ?store.current(), "remote flags settled");
    if let Some(flags) = fetched {
        let entry = CacheEntry {
            fetched_at: now,
            flags,
        };
        cache::write(&source.cache_path, &entry).await;
    }
}

/// Start the launch refresh against the shipped URL as a task of its
/// own, so startup never waits on it.
pub fn spawn_launch_refresh(
    store: FlagStore,
    client: reqwest::Client,
    cache_path: PathBuf,
) -> tokio::task::JoinHandle<()> {
    let source = Source {
        url: FLAGS_URL.to_string(),
        cache_path,
        timeout: FETCH_TIMEOUT,
    };
    tokio::spawn(async move {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        refresh(&store, &client, &source, now).await;
    })
}

#[cfg(test)]
#[path = "store_test.rs"]
mod tests;
