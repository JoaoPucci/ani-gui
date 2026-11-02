//! Writing a full-list snapshot into `user_list_cache`. Split out of
//! `cache.rs` so that file stays inside the CRAP gate's per-file bar.

use std::collections::HashSet;

use rusqlite::params;

use super::{now_secs, provider_slug};
use crate::account::provider::{ListEntry, ProviderKind};
use crate::cache::SqlitePool;
use crate::commands::account::status_to_snake;
use crate::error::{AniError, Result};

/// Replace the cached list for `(provider, user_id)` with the supplied
/// snapshot. Existing rows for the user are deleted first so an entry
/// the user removed from their provider's list (which therefore won't
/// appear in `entries`) doesn't linger in the cache and surface in the
/// Watch Later rail. Both the DELETE and the inserts run in one
/// transaction so a partial failure can't leave the cache half-rebuilt.
pub fn write_entries(
    pool: &SqlitePool,
    kind: ProviderKind,
    user_id: &str,
    entries: &[ListEntry],
) -> Result<()> {
    write_entries_keeping(pool, kind, user_id, entries, &HashSet::new())
}

/// [`write_entries`], leaving the rows of the media in `keep` as they are
/// — present or absent — whatever `entries` says of them: a write landed
/// on those while the snapshot was being read, and its answer is newer.
pub fn write_entries_keeping(
    pool: &SqlitePool,
    kind: ProviderKind,
    user_id: &str,
    entries: &[ListEntry],
    keep: &HashSet<u32>,
) -> Result<()> {
    let mut conn = pool.get().map_err(|_| AniError::Cache)?;
    let now = now_secs();
    let tx = conn.transaction().map_err(|_| AniError::Cache)?;
    {
        // Drop stale rows first — anything the user removed upstream
        // (or that aged out for any other reason) goes here. Without
        // this, a removed AniList entry would survive in the cache
        // until disconnect cleared the whole user table.
        let kept: Vec<i64> = keep.iter().map(|&id| i64::from(id)).collect();
        tx.execute(
            "DELETE FROM user_list_cache WHERE provider = ?1 AND user_id = ?2 \
             AND media_id NOT IN (SELECT value FROM json_each(?3))",
            params![
                provider_slug(kind),
                user_id,
                serde_json::to_string(&kept).map_err(|_| AniError::Cache)?
            ],
        )
        .map_err(|_| AniError::Cache)?;
        let mut stmt = tx
            .prepare(
                "INSERT OR REPLACE INTO user_list_cache \
                 (provider, user_id, media_id, mal_id, status, progress, \
                  score_x100, updated_at, fetched_at, title) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .map_err(|_| AniError::Cache)?;
        for e in entries.iter().filter(|e| !keep.contains(&e.media_id.0)) {
            stmt.execute(params![
                provider_slug(kind),
                user_id,
                i64::from(e.media_id.0),
                e.mal_id.map(i64::from),
                status_to_snake(e.status),
                i64::from(e.progress_episodes),
                e.score_0_to_100.map(i64::from),
                e.updated_at_epoch_s,
                now,
                e.title,
            ])
            .map_err(|_| AniError::Cache)?;
        }
    }
    tx.commit().map_err(|_| AniError::Cache)?;
    Ok(())
}
