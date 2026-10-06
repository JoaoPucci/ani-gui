//! Refinery-managed schema migrations for the SQLite cache.
//!
//! Migration files live under `backend/migrations/` and are
//! embedded into the binary at compile time by `refinery::embed_migrations!`.
//! Add new versions as `V<N>__<name>.sql` files; refinery enforces strict
//! ordering through its `refinery_schema_history` table.

use rusqlite::Connection;

use crate::error::{AniError, Result};

mod embedded {
    refinery::embed_migrations!("./migrations");
}

/// Run all pending migrations against the given connection. Idempotent.
///
/// # Errors
/// Returns [`AniError::Cache`] when refinery rejects a migration (typically
/// schema drift between an older DB on disk and the latest embedded SQL).
pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    embedded::migrations::runner()
        .run(conn)
        .map(|_| ())
        .map_err(|_| AniError::Cache)
}

/// Run the migrations up to and including `version`, as a cache an
/// older build left on disk would have them. Tests stage such a cache
/// to check what a later migration does to it.
///
/// # Errors
/// As [`run_migrations`].
#[cfg(test)]
pub(crate) fn run_migrations_to(conn: &mut Connection, version: u32) -> Result<()> {
    embedded::migrations::runner()
        .set_target(refinery::Target::Version(version))
        .run(conn)
        .map(|_| ())
        .map_err(|_| AniError::Cache)
}
