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

use rusqlite::{params, Connection};

use crate::commands::play_native_versions::VersionKind;
use crate::error::{AniError, Result};

/// The play family at v28: `play:v28:<title>:<mode>:<quality>:
/// <episode>:<year>:<count>:<subtype>`, then `:<version>` for a request
/// naming one. v29 took the quality out, since every play resolves the
/// master. v28 never shipped in a release (0.17.0 keyed its rows at
/// v26), so only builds made from master since then carry such rows;
/// a released install's rows go to the reclaim and resolve afresh.
const QUALITY_KEYED: &str = "play:v28:";

/// The key this build reads for the play row an earlier build kept at
/// `old`, or `None` when the row is not carried: a v28 row kept for
/// `best` — the master — moves to the same request's key without the
/// quality; a row kept for a height or `worst` holds one rendition,
/// which the player could not choose a level of, and stays behind for
/// the reclaim. The title may carry colons, so the fields are read
/// from the right, where none can.
#[must_use]
pub(crate) fn rekeyed(old: &str) -> Option<String> {
    let rest = old.strip_prefix(QUALITY_KEYED)?;
    let (rest, version) = match rest.rsplit_once(':') {
        Some((head, v))
            if v == VersionKind::Standard.as_str() || v == VersionKind::Uncensored.as_str() =>
        {
            (head, Some(v))
        }
        _ => (rest, None),
    };
    let mut fields = rest.rsplitn(7, ':');
    let subtype = fields.next()?;
    let count = fields.next()?;
    let year = fields.next()?;
    let episode = fields.next()?;
    let quality = fields.next()?;
    let mode = fields.next()?;
    let title = fields.next()?;
    if quality != crate::scraper::hls::MASTER || !matches!(mode, "sub" | "dub") {
        return None;
    }
    let key = format!(
        "{}{title}:{mode}:{episode}:{year}:{count}:{subtype}",
        crate::cache::families::play()
    );
    Some(match version {
        None => key,
        Some(v) => format!("{key}:{v}"),
    })
}

/// Copy every carried play row to its key under this build's family,
/// with its moment and lifetime, keeping a row this build already
/// wrote there. The rows left behind — those not carried, and any the
/// read refuses — are deleted by the launch's reclaim, which no longer
/// declares their family.
///
/// # Errors
/// [`AniError::Cache`] when SQLite refuses the read or a write.
pub(crate) fn carry_play_rows(conn: &mut Connection) -> Result<()> {
    let rows: Vec<(String, String, i64, i64)> = {
        let mut stmt = conn
            .prepare(
                "SELECT key, body, fetched_at, ttl_seconds FROM meta_cache \
                 WHERE substr(key, 1, 9) = ?1",
            )
            .map_err(|_| AniError::Cache)?;
        let found = stmt
            .query_map([QUALITY_KEYED], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .map_err(|_| AniError::Cache)?;
        // A row the read refuses (a body stored as a blob) is left for
        // the reclaim, like any row that is not carried.
        found.filter_map(std::result::Result::ok).collect()
    };
    let tx = conn.transaction().map_err(|_| AniError::Cache)?;
    for (key, body, fetched_at, ttl) in rows {
        let Some(carried) = rekeyed(&key) else {
            continue;
        };
        tx.execute(
            "INSERT OR IGNORE INTO meta_cache(key, body, fetched_at, ttl_seconds) \
             VALUES (?1, ?2, ?3, ?4)",
            params![carried, body, fetched_at, ttl],
        )
        .map_err(|_| AniError::Cache)?;
    }
    tx.commit().map_err(|_| AniError::Cache)
}

#[cfg(test)]
#[path = "play_rekey_test.rs"]
mod tests;
