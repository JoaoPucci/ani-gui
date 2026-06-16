//! Single-row write-through into `user_list_cache`.
//!
//! Split out of [`crate::account::cache`] so that module's intrinsic
//! cyclomatic complexity stays under the CRAP ratchet ceiling — the
//! bulk read/replace helpers already saturate it. This file owns just
//! the one-row upsert used by the write-back path.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;

use crate::account::provider::{ListEntry, ProviderKind};
use crate::cache::SqlitePool;
use crate::commands::account::status_to_snake;
use crate::error::{AniError, Result};

fn now_secs() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    )
    .unwrap_or(0)
}

/// Write a single entry back into the cache for its
/// `(provider, user_id, media_id)`, leaving every other row untouched.
/// Used after a tracker write so the cached status reflects the change
/// immediately — a Plan-to-Watch title started via mark-watched flips
/// to Watching and drops out of the Watch Later rail's planning filter
/// without waiting for a full resync (Codex P2 #3412673593). Unlike
/// [`crate::account::cache::write_entries`], this does NOT clear the
/// rest of the user's list.
///
/// Monotonic on `progress` (Codex P2 #3416732383): the write-through
/// runs outside `push_progress`'s per-show lock, so two concurrent
/// mark-watched writes can land in either order. The `ON CONFLICT`
/// guard keeps the higher progress, so a stale lower-progress write
/// can't regress the rail. A genuinely new row still inserts.
pub fn upsert_entry(
    pool: &SqlitePool,
    kind: ProviderKind,
    user_id: &str,
    entry: &ListEntry,
) -> Result<()> {
    upsert(pool, kind, user_id, entry, false)
}

/// Like [`upsert_entry`] but overwrites unconditionally — no monotonic
/// progress guard. Used by the explicit detail-page list editor, where
/// the user can deliberately correct an over-count *downward*; the
/// guarded variant would swallow that lower value. The automatic
/// mark-watched write-through keeps [`upsert_entry`].
pub fn upsert_entry_force(
    pool: &SqlitePool,
    kind: ProviderKind,
    user_id: &str,
    entry: &ListEntry,
) -> Result<()> {
    upsert(pool, kind, user_id, entry, true)
}

/// Delete the single cache row for `(provider, user_id, media_id)`.
/// Used by the explicit "Remove from list" path so the rail/editor stop
/// showing a just-removed show without a full resync. A missing row is a
/// no-op (0 rows affected).
pub fn delete_entry_row(
    pool: &SqlitePool,
    kind: ProviderKind,
    user_id: &str,
    media_id: u32,
) -> Result<()> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    conn.execute(
        "DELETE FROM user_list_cache \
         WHERE provider = ?1 AND user_id = ?2 AND media_id = ?3",
        params![kind.slug(), user_id, i64::from(media_id)],
    )
    .map_err(|_| AniError::Cache)?;
    Ok(())
}

/// Shared upsert. `force` drops the `WHERE excluded.progress >= …`
/// guard so an explicit edit can lower the cached progress.
fn upsert(
    pool: &SqlitePool,
    kind: ProviderKind,
    user_id: &str,
    entry: &ListEntry,
    force: bool,
) -> Result<()> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    let guard = if force {
        ""
    } else {
        // Monotonic on progress so two racing mark-watched writes can't
        // regress the count (Codex P2 #3416732383). Coordination with the
        // explicit editor's force-upsert is handled by the per-show lock:
        // push_progress now writes the cache under that lock (see
        // push_progress_via), so a stale write can't land after an
        // explicit correction.
        " WHERE excluded.progress >= user_list_cache.progress"
    };
    let sql = format!(
        "INSERT INTO user_list_cache \
         (provider, user_id, media_id, mal_id, status, progress, \
          score_x100, updated_at, fetched_at, title) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
         ON CONFLICT(provider, user_id, media_id) DO UPDATE SET \
            mal_id = excluded.mal_id, status = excluded.status, \
            progress = excluded.progress, score_x100 = excluded.score_x100, \
            updated_at = excluded.updated_at, fetched_at = excluded.fetched_at, \
            title = excluded.title{guard}"
    );
    conn.execute(
        &sql,
        params![
            kind.slug(),
            user_id,
            i64::from(entry.media_id.0),
            entry.mal_id.map(i64::from),
            status_to_snake(entry.status),
            i64::from(entry.progress_episodes),
            entry.score_0_to_100.map(i64::from),
            entry.updated_at_epoch_s,
            now_secs(),
            entry.title,
        ],
    )
    .map_err(|_| AniError::Cache)?;
    Ok(())
}

#[cfg(test)]
#[path = "cache_upsert_test.rs"]
mod tests;
