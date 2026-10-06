//! The rules a lookup's write follows against the refresh map — when it
//! may write, and holding the row lock while the write is still its
//! own; split from [`super`] so each file stays inside the CRAP gate's
//! per-file bar.

use super::*;

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
        refreshes.generation(&crate::commands::availability::cache_key(id, mode))
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
