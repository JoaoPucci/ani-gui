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

/// Write a single entry back into the cache, replacing the existing row
/// for its `(provider, user_id, media_id)` and leaving every other row
/// untouched. Used after a tracker write so the cached status reflects
/// the change immediately — a Plan-to-Watch title started via
/// mark-watched flips to Watching and drops out of the Watch Later
/// rail's planning filter without waiting for a full resync (Codex P2
/// #3412673593). Unlike [`crate::account::cache::write_entries`], this
/// does NOT clear the rest of the user's list.
pub fn upsert_entry(
    pool: &SqlitePool,
    kind: ProviderKind,
    user_id: &str,
    entry: &ListEntry,
) -> Result<()> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    conn.execute(
        "INSERT OR REPLACE INTO user_list_cache \
         (provider, user_id, media_id, mal_id, status, progress, \
          score_x100, updated_at, fetched_at, title) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
