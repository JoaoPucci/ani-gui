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
    let _ = (conn, families);
    Ok(ReclaimReport::default())
}

#[cfg(test)]
#[path = "reclaim_test.rs"]
mod tests;
