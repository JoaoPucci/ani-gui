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
use crate::commands::account::status_from_snake;
use crate::error::{AniError, Result};

#[path = "cache_list.rs"]
mod list;
pub use list::list_entries;

#[path = "cache_write.rs"]
mod write;
pub use write::{write_entries, write_entries_keeping};

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

/// Delete every row for `provider` regardless of `user_id`. When `hydrate()`
/// puts the provider in the unreadable-token error state (orphan token file, no
/// decoded account), the renderer's safeStorage has no `user_id` to scope the
/// per-user clear, so the standard delete-cache path can't run. The frontend
/// calls this provider-wide flavour as the cleanup step before dropping the
/// orphan file. Still gated by the renderer-only internal secret at the API
/// boundary — a cross-origin tab can't trigger it without knowing the 32-byte
/// secret.
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
