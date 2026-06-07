//! Connection pool + typed repos for the SQLite metadata cache.
//!
//! The pool wraps `r2d2_sqlite::SqliteConnectionManager`. On open, all
//! pending refinery migrations are applied (idempotent). Repos are free
//! functions that take `&SqlitePool` so they don't have to thread a
//! connection through the call site.
//!
//! ## TTL policy
//!
//! `meta_cache_get` returns `None` for entries past their TTL but does
//! not delete them — overwrite happens on the next `meta_cache_put`. This
//! lets a future revalidation flow opt to serve the stale body while a
//! background refresh runs.
//!
//! ## Threading
//!
//! Calls are synchronous. From async contexts, wrap in
//! `tokio::task::spawn_blocking` to avoid stalling the runtime.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OptionalExtension};

use crate::cache::schema::run_migrations;
use crate::error::{AniError, Result};

/// Convenience type alias for the pool.
pub type SqlitePool = Pool<SqliteConnectionManager>;

/// Open a pool against an on-disk SQLite database, creating the file if
/// it doesn't exist. Runs all pending migrations.
///
/// # Errors
/// - [`AniError::Cache`] when the pool can't be built or migrations fail.
pub fn open_pool(path: &Path) -> Result<SqlitePool> {
    let manager = SqliteConnectionManager::file(path);
    let pool = Pool::builder()
        .max_size(4)
        .build(manager)
        .map_err(|_| AniError::Cache)?;
    let mut conn = pool.get().map_err(|_| AniError::Cache)?;
    run_migrations(&mut conn)?;
    Ok(pool)
}

/// Open a pool against an in-memory database. Tests only — `:memory:` is
/// per-connection in SQLite, so the pool is forced to `max_size(1)` so
/// the migration writes are visible to subsequent reads.
///
/// # Errors
/// - [`AniError::Cache`] when the pool can't be built or migrations fail.
pub fn open_in_memory() -> Result<SqlitePool> {
    let manager = SqliteConnectionManager::memory();
    let pool = Pool::builder()
        .max_size(1)
        .build(manager)
        .map_err(|_| AniError::Cache)?;
    let mut conn = pool.get().map_err(|_| AniError::Cache)?;
    run_migrations(&mut conn)?;
    Ok(pool)
}

// --- meta_cache ----------------------------------------------------------

/// Fetch a meta_cache body if present and not expired.
///
/// # Errors
/// [`AniError::Cache`] on connection or query failure.
pub fn meta_cache_get(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    let row: Option<(String, i64, i64)> = conn
        .query_row(
            "SELECT body, fetched_at, ttl_seconds FROM meta_cache WHERE key = ?1",
            params![key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|_| AniError::Cache)?;
    Ok(row.and_then(|(body, fetched_at, ttl)| {
        // `>=` so `ttl_seconds == 0` means "immediately expired"; the
        // common interpretation of a zero TTL across HTTP and most
        // cache libs.
        if now_secs().saturating_sub(fetched_at) >= ttl {
            None
        } else {
            Some(body)
        }
    }))
}

/// Insert or replace a meta_cache entry. `ttl_seconds` controls the
/// freshness window enforced by [`meta_cache_get`].
///
/// # Errors
/// [`AniError::Cache`] on write failure.
pub fn meta_cache_put(pool: &SqlitePool, key: &str, body: &str, ttl_seconds: u64) -> Result<()> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    let ttl_i64 = i64::try_from(ttl_seconds).unwrap_or(i64::MAX);
    conn.execute(
        "INSERT OR REPLACE INTO meta_cache(key, body, fetched_at, ttl_seconds) \
         VALUES (?1, ?2, ?3, ?4)",
        params![key, body, now_secs(), ttl_i64],
    )
    .map_err(|_| AniError::Cache)?;
    Ok(())
}

/// Delete every meta_cache entry. Used by tests and a future "clear cache"
/// menu item.
///
/// # Errors
/// [`AniError::Cache`] on write failure.
pub fn meta_cache_clear(pool: &SqlitePool) -> Result<()> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    conn.execute("DELETE FROM meta_cache", [])
        .map_err(|_| AniError::Cache)?;
    Ok(())
}

/// Delete a single meta_cache entry. Used by feedback eviction (a
/// cached play resolution that the player just failed to load — drop
/// it so the next attempt re-fetches from upstream).
///
/// # Errors
/// [`AniError::Cache`] on write failure.
pub fn meta_cache_delete(pool: &SqlitePool, key: &str) -> Result<()> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    conn.execute("DELETE FROM meta_cache WHERE key = ?1", params![key])
        .map_err(|_| AniError::Cache)?;
    Ok(())
}

/// List every (key, body) pair under `prefix`, dropping expired rows.
/// Used by the watched-at endpoint to return the full per-show stamp
/// map in one query rather than N round-trips.
///
/// # Errors
/// [`AniError::Cache`] on connection or query failure.
pub fn meta_cache_list_prefix(pool: &SqlitePool, prefix: &str) -> Result<Vec<(String, String)>> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    // SQL LIKE: escape the literal `%` and `_` chars in the caller's
    // prefix so a key prefix like `play:v2:` doesn't accidentally
    // match other underscore patterns. ESCAPE '\\' lets us mark them.
    let escaped = prefix
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("{escaped}%");
    let mut stmt = conn
        .prepare(
            "SELECT key, body, fetched_at, ttl_seconds FROM meta_cache \
             WHERE key LIKE ?1 ESCAPE '\\'",
        )
        .map_err(|_| AniError::Cache)?;
    let rows = stmt
        .query_map(params![pattern], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .map_err(|_| AniError::Cache)?;
    let now = now_secs();
    let mut out = Vec::new();
    for row in rows {
        let (key, body, fetched_at, ttl) = row.map_err(|_| AniError::Cache)?;
        if now.saturating_sub(fetched_at) < ttl {
            out.push((key, body));
        }
    }
    Ok(out)
}

// --- title_match ---------------------------------------------------------

/// One row of the title_match table: a normalized user query string
/// resolved to one or both metadata-source ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleMatch {
    /// Lowercased + whitespace-collapsed user query.
    pub query_norm: String,
    /// Kitsu anime id (string in JSON:API).
    pub kitsu_id: Option<String>,
    /// AniList anime id (integer in GraphQL).
    pub anilist_id: Option<i64>,
    /// Unix epoch seconds the entry was last refreshed.
    pub fetched_at: i64,
}

/// Fetch a title_match row by normalized query.
///
/// # Errors
/// [`AniError::Cache`] on query failure.
pub fn title_match_get(pool: &SqlitePool, query_norm: &str) -> Result<Option<TitleMatch>> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    conn.query_row(
        "SELECT query_norm, kitsu_id, anilist_id, fetched_at FROM title_match WHERE query_norm = ?1",
        params![query_norm],
        |r| {
            Ok(TitleMatch {
                query_norm: r.get(0)?,
                kitsu_id: r.get(1)?,
                anilist_id: r.get(2)?,
                fetched_at: r.get(3)?,
            })
        },
    )
    .optional()
    .map_err(|_| AniError::Cache)
}

/// Insert or replace a title_match row.
///
/// # Errors
/// [`AniError::Cache`] on write failure.
pub fn title_match_put(
    pool: &SqlitePool,
    query_norm: &str,
    kitsu_id: Option<&str>,
    anilist_id: Option<i64>,
) -> Result<()> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    conn.execute(
        "INSERT OR REPLACE INTO title_match(query_norm, kitsu_id, anilist_id, fetched_at) \
         VALUES (?1, ?2, ?3, ?4)",
        params![query_norm, kitsu_id, anilist_id, now_secs()],
    )
    .map_err(|_| AniError::Cache)?;
    Ok(())
}

// --- helpers -------------------------------------------------------------

fn now_secs() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    )
    .unwrap_or(0)
}

#[cfg(test)]
#[path = "db_test.rs"]
mod tests;
