//! Mark the reverse mappings a play stored before plays marked them.
//!
//! A play stores the `show key → kitsu id` mapping
//! (`allmanga2kitsu:v3:<show>`) with a mark beside it naming the id it
//! stored (`allmanga2kitsu:played:v1:<show>`); a mapping is the play's
//! while the mark names it ([`crate::commands::kitsu_played`]). Earlier
//! builds left no mark, and read a mapping as the play's when it was
//! written from a second before to ten seconds after the show's watch
//! stamp (`watched-at:v1:<show>`, epoch milliseconds), both rows
//! unexpired. Opening the cache marks every mapping that rule read as
//! played, so a play stored before the upgrade keeps its standing. The
//! mark takes the mapping's moment and lifetime, and expires with it.
//!
//! It runs once per cache file, gated by SQLite's `user_version`, which
//! clearing the cache's rows leaves standing: a rerun would read the
//! rule into mappings written since, which the marks judge alone. It is
//! not a schema migration, because refinery aborts on an applied
//! migration it does not have, and one would stop an earlier build from
//! opening the cache after a downgrade. An earlier build ignores both
//! the marks and `user_version`.

use rusqlite::{Connection, TransactionBehavior};

use crate::error::{AniError, Result};

/// The `user_version` a cache whose earlier plays are marked carries.
const MARKED: i64 = 1;

const MARK_EARLIER_PLAYS: &str = "\
INSERT OR REPLACE INTO meta_cache(key, body, fetched_at, ttl_seconds)
SELECT 'allmanga2kitsu:played:v1:' || substr(m.key, 19),
       m.body,
       m.fetched_at,
       m.ttl_seconds
FROM meta_cache AS m
JOIN meta_cache AS w
  ON w.key = 'watched-at:v1:' || substr(m.key, 19)
WHERE substr(m.key, 1, 18) = 'allmanga2kitsu:v3:'
  AND CAST(strftime('%s', 'now') AS INTEGER) - m.fetched_at < m.ttl_seconds
  AND CAST(strftime('%s', 'now') AS INTEGER) - w.fetched_at < w.ttl_seconds
  AND w.body <> ''
  AND w.body NOT GLOB '*[^0-9]*'
  AND m.fetched_at * 1000 - CAST(w.body AS INTEGER) BETWEEN -1000 AND 10000";

/// Mark the mappings earlier builds read as played, unless this cache
/// already has been. The marks and the gate land together or not at
/// all.
///
/// # Errors
/// [`AniError::Cache`] when SQLite refuses the read or the write.
pub(crate) fn mark_earlier_plays(conn: &mut Connection) -> Result<()> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| AniError::Cache)?;
    let version: i64 = tx
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|_| AniError::Cache)?;
    if version >= MARKED {
        return Ok(());
    }
    tx.execute(MARK_EARLIER_PLAYS, [])
        .map_err(|_| AniError::Cache)?;
    tx.pragma_update(None, "user_version", MARKED)
        .map_err(|_| AniError::Cache)?;
    tx.commit().map_err(|_| AniError::Cache)
}
