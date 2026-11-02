//! The last good flags document, kept on disk with the time it was
//! fetched.
//!
//! It stands in for the remote file when a launch cannot read it —
//! offline, a timeout, an error status, a malformed body — for as long
//! as it is fresh ([`FRESH_FOR`]). Past that, the built-in defaults
//! apply. The entry holds the document rather than resolved values,
//! so it goes through the same resolution as a fetched file: a cache
//! written by another build, or edited by hand, still resolves only to
//! values this build declares.
//!
//! **Why seven days.** Every value a document can resolve to is a
//! flag's default or a declared retreat from it, so a cached document
//! is never less safe than the defaults it stands in for; what its age
//! bounds is how long a retreat the maintainer has since lifted
//! lingers on a machine that cannot reach GitHub. A week rides out a
//! run of launches without the network — a trip, a captive portal, a
//! blocked CDN — so a feature switched off for a bad release stays off
//! through them, while a lifted retreat clears within a week even on a
//! machine that never reaches the file again. An entry dated in the
//! future (a clock set back) is not fresh: it could otherwise outlive
//! the window by however far the clock moved.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::resolve::{Document, MAX_BYTES};

/// How long a cached document stands in for an unreachable file.
pub const FRESH_FOR: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// What the cache file holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheEntry {
    /// When the document was fetched, in seconds since the Unix epoch.
    pub fetched_at: u64,
    /// The `flags` object as fetched.
    pub flags: Document,
}

/// Whether an entry fetched at `fetched_at` may stand in at `now`:
/// not dated after `now`, and no older than `window`.
#[must_use]
pub fn is_fresh(fetched_at: u64, now: u64, window: Duration) -> bool {
    let _ = (fetched_at, now, window);
    false
}

/// Read the cache file. `None` when it is missing, unreadable, larger
/// than a document plus its envelope could be, or not an entry.
pub async fn read(path: &Path) -> Option<CacheEntry> {
    let _ = path;
    None
}

/// Replace the cache file with `entry`, through a temporary file so a
/// launch ended mid-write never leaves a torn one. A failure is
/// logged and otherwise ignored: the cache is a convenience.
pub async fn write(path: &Path, entry: &CacheEntry) {
    let _ = (path, entry, MAX_BYTES);
}

#[cfg(test)]
#[path = "cache_test.rs"]
mod tests;
