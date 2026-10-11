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

#[path = "cache_list.rs"]
mod list;
pub use list::list_entries;

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
