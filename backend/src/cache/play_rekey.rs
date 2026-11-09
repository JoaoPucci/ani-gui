//! Carry the play resolutions an earlier build keyed across a change
//! of the key's shape.
//!
//! A play row's key names what the request asked for
//! ([`crate::commands::play_resolution_cache::cache_key`]). When an
//! axis leaves the key, the rows written under it are still good
//! answers for the requests that read them, so opening the cache
//! copies each one to the key this build reads, with its moment and
//! lifetime, rather than letting the launch's reclaim drop it and the
//! next play resolve afresh. A row that is no longer a good answer is
//! left where it is, and the reclaim then deletes it with the rest of
//! its family.

use rusqlite::Connection;

use crate::error::Result;

/// The key this build reads for the play row an earlier build kept at
/// `old`, or `None` when the row is not carried.
#[must_use]
pub(crate) fn rekeyed(old: &str) -> Option<String> {
    let _ = old;
    None
}

/// Copy every carried play row to its key under this build's family,
/// keeping a row this build already wrote there.
///
/// # Errors
/// [`crate::error::AniError::Cache`] when SQLite refuses the read or
/// a write.
pub(crate) fn carry_play_rows(conn: &mut Connection) -> Result<()> {
    let _ = conn;
    Ok(())
}

#[cfg(test)]
#[path = "play_rekey_test.rs"]
mod tests;
