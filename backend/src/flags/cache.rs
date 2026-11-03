//! The last good flags document, kept on disk with the time it was
//! fetched.
//!
//! It stands in for the remote file when a launch cannot read it —
//! offline, a timeout, an error status, a malformed body — for as long
//! as it is fresh ([`FRESH_FOR`]). Past that, the built-in defaults
//! apply. The entry holds the document rather than resolved values,
//! so it goes through the same resolution as a fetched file: a cache
//! written by another build, or edited by hand, still resolves only to
//! values this build declares. The notices block it may hold is kept
//! only if it verified when fetched (see [`super::signature`]); it is
//! not checked again on reading, so a hand edit of the user's own cache
//! reaches only that user's notices.
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
    now.checked_sub(fetched_at)
        .is_some_and(|age| age <= window.as_secs())
}

/// The largest cache file read: a document at its cap, plus room for
/// the envelope and the escaping a re-serialised document can add.
const MAX_FILE_BYTES: u64 = 4 * MAX_BYTES as u64;

/// Read the cache file. `None` when it is missing, not a regular file,
/// unreadable, larger than a document plus its envelope could be, or
/// not an entry.
pub async fn read(path: &Path) -> Option<CacheEntry> {
    use tokio::io::AsyncReadExt;
    // Not followed: a link, a device or a pipe is not a cache file, and
    // a device or a pipe reports a length of 0 that a size check would
    // trust.
    if !tokio::fs::symlink_metadata(path).await.ok()?.is_file() {
        return None;
    }
    let file = tokio::fs::File::open(path).await.ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .ok()?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

/// Replace the cache file with `entry`, through a temporary file so a
/// launch ended mid-write never leaves a torn one. A failure is
/// logged and otherwise ignored: the cache is a convenience.
pub async fn write(path: &Path, entry: &CacheEntry) {
    if let Err(error) = try_write(path, entry).await {
        tracing::warn!(%error, path = %path.display(), "could not cache the flags file");
    }
}

async fn try_write(path: &Path, entry: &CacheEntry) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let bytes = serde_json::to_vec(entry)?;
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, &bytes).await?;
    tokio::fs::rename(&tmp, path).await
}

#[cfg(test)]
#[path = "cache_test.rs"]
mod tests;
