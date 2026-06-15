//! DB helpers over the `user_list_cache` table (V002 migration).
//!
//! Kept separate from [`crate::cache::db`] because that module hosts
//! the V001 surfaces (`meta_cache`, `title_match`, `image_index`) and
//! its CCN is already substantial. This module owns just the account
//! table.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;

use crate::account::provider::{ListEntry, ProviderKind, ProviderMediaId};
use crate::cache::SqlitePool;
use crate::commands::account::{status_from_snake, status_to_snake};
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

fn provider_slug(kind: ProviderKind) -> &'static str {
    kind.slug()
}

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
    let mut conn = pool.get().map_err(|_| AniError::Cache)?;
    let now = now_secs();
    let tx = conn.transaction().map_err(|_| AniError::Cache)?;
    {
        // Drop stale rows first — anything the user removed upstream
        // (or that aged out for any other reason) goes here. Without
        // this, a removed AniList entry would survive in the cache
        // until disconnect cleared the whole user table.
        tx.execute(
            "DELETE FROM user_list_cache WHERE provider = ?1 AND user_id = ?2",
            params![provider_slug(kind), user_id],
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
        for e in entries {
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

/// Write a single entry back into the cache, replacing the existing
/// row for its `(provider, user_id, media_id)` and leaving every other
/// row untouched. Used after a tracker write so the cached status
/// reflects the change immediately — e.g. a Plan-to-Watch title started
/// via mark-watched flips to Watching and drops out of the Watch Later
/// rail's planning filter without waiting for a full resync (Codex P2
/// #3412673593). Unlike [`write_entries`], this does NOT clear the rest
/// of the user's list.
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
            provider_slug(kind),
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

/// Read every cached entry for `(provider, user_id)`. Used by the
/// home Watch Later rail (PR #2) and the /account page stats.
pub fn list_entries(
    pool: &SqlitePool,
    kind: ProviderKind,
    user_id: &str,
) -> Result<Vec<ListEntry>> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    let mut stmt = conn
        .prepare(
            "SELECT media_id, mal_id, status, progress, score_x100, updated_at, title \
             FROM user_list_cache \
             WHERE provider = ?1 AND user_id = ?2",
        )
        .map_err(|_| AniError::Cache)?;
    let rows = stmt
        .query_map(params![provider_slug(kind), user_id], |r| {
            let media_id: i64 = r.get(0)?;
            let mal_id: Option<i64> = r.get(1)?;
            let status: String = r.get(2)?;
            let progress: i64 = r.get(3)?;
            let score: Option<i64> = r.get(4)?;
            let updated_at: i64 = r.get(5)?;
            let title: Option<String> = r.get(6)?;
            Ok((media_id, mal_id, status, progress, score, updated_at, title))
        })
        .map_err(|_| AniError::Cache)?;
    let mut out = Vec::new();
    for row in rows {
        let (media_id, mal_id, status_s, progress, score, updated_at, title) =
            row.map_err(|_| AniError::Cache)?;
        // Unknown status strings (e.g. a future provider that adds a
        // status we don't know yet) get dropped rather than blowing up
        // the whole list read. Logging that in detail is overkill for
        // this path.
        let Some(status) = status_from_snake(&status_s) else {
            continue;
        };
        out.push(ListEntry {
            provider: kind,
            media_id: ProviderMediaId(media_id as u32),
            mal_id: mal_id.map(|v| v as u32),
            status,
            progress_episodes: progress as u32,
            score_0_to_100: score.map(|v| v.clamp(0, 100) as u8),
            updated_at_epoch_s: updated_at,
            title: title.unwrap_or_default(),
        });
    }
    Ok(out)
}

/// Delete every row for `(provider, user_id)`. Called on disconnect.
pub fn clear_user(pool: &SqlitePool, kind: ProviderKind, user_id: &str) -> Result<()> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    conn.execute(
        "DELETE FROM user_list_cache WHERE provider = ?1 AND user_id = ?2",
        params![provider_slug(kind), user_id],
    )
    .map_err(|_| AniError::Cache)?;
    Ok(())
}

/// Delete every row for `provider` regardless of `user_id`. Codex P2
/// #3371658227: when `hydrate()` puts the provider in the unreadable-
/// token error state (orphan token file, no decoded account), the
/// renderer's safeStorage has no `user_id` to scope the per-user
/// clear, so the standard delete-cache path can't run. The frontend
/// calls this provider-wide flavour as the cleanup step before
/// dropping the orphan file. Still gated by the renderer-only
/// internal secret at the API boundary — a cross-origin tab can't
/// trigger it without knowing the 32-byte secret.
pub fn clear_provider(pool: &SqlitePool, kind: ProviderKind) -> Result<()> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    conn.execute(
        "DELETE FROM user_list_cache WHERE provider = ?1",
        params![provider_slug(kind)],
    )
    .map_err(|_| AniError::Cache)?;
    Ok(())
}

#[cfg(test)]
#[path = "cache_test.rs"]
mod tests;
