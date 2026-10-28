//! Mark the reverse mappings a play stored before plays marked them.
//!
//! A play stores the `show key → kitsu id` mapping
//! (`allmanga2kitsu:v<N>:<show>`, at
//! [`crate::commands::kitsu::ALLMANGA_KITSU_VERSION`]) with a mark
//! beside it naming the id it stored
//! (`allmanga2kitsu:played:v1:<show>`); a mapping is the play's while
//! the mark names it ([`crate::commands::kitsu_played`]). Earlier
//! builds left no mark: they stored mappings at v3, the version this
//! pass reads, and read a mapping as the play's when it was written
//! from a second before to ten seconds after the show's watch stamp
//! (`watched-at:v1:<show>`, epoch milliseconds), both rows unexpired.
//! Opening the cache marks every mapping that rule read as played, so
//! a play stored before the upgrade keeps its standing while its
//! mapping is read. The mark takes the mapping's moment and lifetime,
//! and expires with it. Each later re-key — v4 when the entry's titles
//! came to rule the pick, v5 when names came to vouch for a countless
//! or single-video pick, v6 when a countless entry stopped taking a
//! hit its year only tolerated and the probe head's evidence came to
//! reach as far as its five, v7 when an entry with a count and no year
//! came to take only a hit named for it, v8 when a dated entry with a
//! count did too and a dated entry's hentai listing stopped being a
//! pick, v9 when an undated entry's hentai listing did too and no pick
//! took a listing whose page lists no genre — retires the mappings before it: their marks then name rows
//! nothing reads, until they expire, and the next play of a show stores
//! a fresh mapping and mark.
//!
//! The same one-time pass drops the detail rows earlier builds kept
//! with no cover and no banner (see `DROP_BANNERLESS_DETAILS`), moving
//! `user_version` from 1 to 2; a cache already at 1 runs that step
//! alone. A cache an earlier build opens again after this one, at 2,
//! is not swept a second time: rows that build keeps for a failed
//! banner lookup stay until they expire.
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

/// The `user_version` a cache carries once the detail rows earlier
/// builds kept with no banner are dropped as well.
const BANNERLESS_DROPPED: i64 = 2;

/// The detail rows earlier builds kept with no cover and no banner.
/// Until the banner backfill told "AniList has no banner" from a lookup
/// that got no answer, a failed lookup was kept this way for up to a
/// week, and nothing in a row tells the two apart: each is dropped, and
/// the show's detail is fetched again. The prefix is the detail key's
/// (`commands::kitsu::anime_detail_key`), which stays put so the
/// history sweep still finds rows by it.
const DROP_BANNERLESS_DETAILS: &str = "\
DELETE FROM meta_cache
WHERE substr(key, 1, 15) = 'kitsu:v5:anime:'
  AND CASE WHEN json_valid(body) THEN
        json_type(body, '$.cover_image') IS NOT 'object'
        AND json_type(body, '$.anilist_banner') IS NOT 'text'
      ELSE 0 END";

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

/// Mark the mappings earlier builds read as played, and drop the detail
/// rows they kept with no banner ([`DROP_BANNERLESS_DETAILS`]), each
/// unless this cache already has been. The steps and the gate land
/// together or not at all, in one write.
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
    if version >= BANNERLESS_DROPPED {
        return Ok(());
    }
    if version < MARKED {
        tx.execute(MARK_EARLIER_PLAYS, [])
            .map_err(|_| AniError::Cache)?;
    }
    tx.execute(DROP_BANNERLESS_DETAILS, [])
        .map_err(|_| AniError::Cache)?;
    tx.pragma_update(None, "user_version", BANNERLESS_DROPPED)
        .map_err(|_| AniError::Cache)?;
    tx.commit().map_err(|_| AniError::Cache)
}
