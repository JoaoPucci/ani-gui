//! Reading a user's cached list back out of `user_list_cache`; split
//! from [`super`] so each file stays inside the CRAP gate's per-file
//! bar.

use super::*;

/// Read every cached entry for `(provider, user_id)`. Used by the
/// home Watch Later rail (PR #2) and the /account page stats.
pub fn list_entries(
    pool: &SqlitePool,
    kind: ProviderKind,
    user_id: &str,
) -> Result<Vec<ListEntry>> {
    let conn = pool.get().map_err(|_| AniError::Cache)?;
    let mut stmt = conn
        .prepare(
            "SELECT media_id, mal_id, status, progress, score_x100, updated_at, title \
             FROM user_list_cache \
             WHERE provider = ?1 AND user_id = ?2",
        )
        .map_err(|_| AniError::Cache)?;
    let rows = stmt
        .query_map(params![provider_slug(kind), user_id], |r| {
            let media_id: i64 = r.get(0)?;
            let mal_id: Option<i64> = r.get(1)?;
            let status: String = r.get(2)?;
            let progress: i64 = r.get(3)?;
            let score: Option<i64> = r.get(4)?;
            let updated_at: i64 = r.get(5)?;
            let title: Option<String> = r.get(6)?;
            Ok((media_id, mal_id, status, progress, score, updated_at, title))
        })
        .map_err(|_| AniError::Cache)?;
    let mut out = Vec::new();
    for row in rows {
        let (media_id, mal_id, status_s, progress, score, updated_at, title) =
            row.map_err(|_| AniError::Cache)?;
        // Unknown status strings (e.g. a future provider that adds a
        // status we don't know yet) get dropped rather than blowing up
        // the whole list read. Logging that in detail is overkill for
        // this path.
        let Some(status) = status_from_snake(&status_s) else {
            continue;
        };
        out.push(ListEntry {
            provider: kind,
            media_id: ProviderMediaId(media_id as u32),
            mal_id: mal_id.map(|v| v as u32),
            status,
            progress_episodes: progress as u32,
            score_0_to_100: score.map(|v| v.clamp(0, 100) as u8),
            updated_at_epoch_s: updated_at,
            title: title.unwrap_or_default(),
        });
    }
    Ok(out)
}
