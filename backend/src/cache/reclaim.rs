//! Deleting at launch the cache rows no reader of this build will ask
//! for: rows under a key family [`super::families`] does not declare —
//! a family re-keyed or retired, its old rows read by nothing — and
//! rows past their lifetime in a family nothing reads past it.
//!
//! Nothing else deletes a cache row once it expires, and a re-key only
//! moves the readers to new keys, so before this both kinds stayed in
//! the file for good. It runs on every launch and keeps no marker: what
//! it looks for is in the rows themselves, so a launch with nothing to
//! reclaim reads the keys and deletes nothing.

use rusqlite::{Connection, TransactionBehavior};
use serde::Serialize;

use super::families::{fate, Family, Fate};
use crate::error::{AniError, Result};

/// What one reclaim deleted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReclaimReport {
    /// Rows under a family this build does not declare.
    pub undeclared_rows: usize,
    /// Rows past their lifetime in a family nothing reads past it.
    pub expired_rows: usize,
}

/// Delete the rows `families` leaves no reader for, in one write.
///
/// # Errors
/// [`AniError::Cache`] when SQLite refuses the read or the write; the
/// transaction then leaves every row where it was.
pub fn reclaim(conn: &mut Connection, families: &[Family]) -> Result<ReclaimReport> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| AniError::Cache)?;
    let now = super::db::now_secs();
    let rows: Vec<(String, bool)> = {
        let mut stmt = tx
            .prepare("SELECT key, fetched_at, ttl_seconds FROM meta_cache")
            .map_err(|_| AniError::Cache)?;
        let rows = stmt
            .query_map([], |r| {
                let (fetched_at, ttl): (i64, i64) = (r.get(1)?, r.get(2)?);
                // The test `meta_cache_get` serves a row by.
                Ok((
                    r.get::<_, String>(0)?,
                    now.saturating_sub(fetched_at) >= ttl,
                ))
            })
            .map_err(|_| AniError::Cache)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| AniError::Cache)?;
        rows
    };
    let mut report = ReclaimReport::default();
    for (key, expired) in rows {
        let counter = match fate(&key, expired, families) {
            Fate::Keep => continue,
            Fate::Undeclared => &mut report.undeclared_rows,
            Fate::Expired => &mut report.expired_rows,
        };
        tx.execute("DELETE FROM meta_cache WHERE key = ?1", [&key])
            .map_err(|_| AniError::Cache)?;
        *counter += 1;
    }
    tx.commit().map_err(|_| AniError::Cache)?;
    Ok(report)
}

#[cfg(test)]
#[path = "reclaim_test.rs"]
mod tests;
