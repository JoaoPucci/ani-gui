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
use crate::commands::account::{status_from_snake, status_to_snake};
use crate::error::{AniError, Result};

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

/// Upsert `entries` into `user_list_cache`. Existing rows for the
/// same `(provider, user_id, media_id)` are replaced — the caller's
/// fresh fetch wins.
pub fn write_entries(
    pool: &SqlitePool,
    kind: ProviderKind,
    user_id: &str,
    entries: &[ListEntry],
) -> Result<()> {
    let mut conn = pool.get().map_err(|_| AniError::Cache)?;
    let now = now_secs();
    let tx = conn.transaction().map_err(|_| AniError::Cache)?;
    {
        let mut stmt = tx
            .prepare(
                "INSERT OR REPLACE INTO user_list_cache \
                 (provider, user_id, media_id, mal_id, status, progress, \
                  score_x100, updated_at, fetched_at, title) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .map_err(|_| AniError::Cache)?;
        for e in entries {
            stmt.execute(params![
                provider_slug(kind),
                user_id,
                i64::from(e.media_id.0),
                e.mal_id.map(i64::from),
                status_to_snake(e.status),
                i64::from(e.progress_episodes),
                e.score_0_to_100.map(i64::from),
                e.updated_at_epoch_s,
                now,
                e.title,
            ])
            .map_err(|_| AniError::Cache)?;
        }
    }
    tx.commit().map_err(|_| AniError::Cache)?;
    Ok(())
}

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
            score_0_to_100: score.map(|v| v.min(100).max(0) as u8),
            updated_at_epoch_s: updated_at,
            title: title.unwrap_or_default(),
        });
    }
    Ok(out)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::status::ListStatus;
    use crate::cache::open_in_memory;

    fn entry(provider: ProviderKind, media_id: u32, status: ListStatus) -> ListEntry {
        ListEntry {
            provider,
            media_id: ProviderMediaId(media_id),
            mal_id: Some(media_id),
            status,
            progress_episodes: 0,
            score_0_to_100: None,
            updated_at_epoch_s: 1_700_000_000,
            title: format!("Show {media_id}"),
        }
    }

    #[test]
    fn write_then_read_round_trips_one_user() {
        let pool = open_in_memory().unwrap();
        let entries = vec![
            entry(ProviderKind::AniList, 1, ListStatus::Planning),
            entry(ProviderKind::AniList, 2, ListStatus::Watching),
        ];
        write_entries(&pool, ProviderKind::AniList, "user-a", &entries).unwrap();
        let got = list_entries(&pool, ProviderKind::AniList, "user-a").unwrap();
        assert_eq!(got.len(), 2);
        let media_ids: Vec<u32> = got.iter().map(|e| e.media_id.0).collect();
        assert!(media_ids.contains(&1));
        assert!(media_ids.contains(&2));
    }

    #[test]
    fn write_replaces_existing_row_for_same_media_id() {
        // Same (provider, user_id, media_id) → upsert wins. Pin the
        // status flips through.
        let pool = open_in_memory().unwrap();
        write_entries(
            &pool,
            ProviderKind::AniList,
            "u",
            &[entry(ProviderKind::AniList, 9, ListStatus::Planning)],
        )
        .unwrap();
        let mut updated = entry(ProviderKind::AniList, 9, ListStatus::Watching);
        updated.progress_episodes = 5;
        write_entries(&pool, ProviderKind::AniList, "u", &[updated]).unwrap();
        let got = list_entries(&pool, ProviderKind::AniList, "u").unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].status, ListStatus::Watching);
        assert_eq!(got[0].progress_episodes, 5);
    }

    #[test]
    fn read_filters_by_provider_and_user() {
        // Two providers + two users → 4 distinct rows; reads of one
        // pair must not leak rows from another.
        let pool = open_in_memory().unwrap();
        write_entries(
            &pool,
            ProviderKind::AniList,
            "u1",
            &[entry(ProviderKind::AniList, 1, ListStatus::Planning)],
        )
        .unwrap();
        write_entries(
            &pool,
            ProviderKind::AniList,
            "u2",
            &[entry(ProviderKind::AniList, 1, ListStatus::Planning)],
        )
        .unwrap();
        assert_eq!(
            list_entries(&pool, ProviderKind::AniList, "u1")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            list_entries(&pool, ProviderKind::AniList, "u2")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            list_entries(&pool, ProviderKind::AniList, "missing")
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn clear_user_drops_only_that_user_rows() {
        let pool = open_in_memory().unwrap();
        write_entries(
            &pool,
            ProviderKind::AniList,
            "u1",
            &[entry(ProviderKind::AniList, 1, ListStatus::Planning)],
        )
        .unwrap();
        write_entries(
            &pool,
            ProviderKind::AniList,
            "u2",
            &[entry(ProviderKind::AniList, 1, ListStatus::Planning)],
        )
        .unwrap();
        clear_user(&pool, ProviderKind::AniList, "u1").unwrap();
        assert_eq!(
            list_entries(&pool, ProviderKind::AniList, "u1")
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            list_entries(&pool, ProviderKind::AniList, "u2")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn unknown_status_in_db_is_silently_skipped_on_read() {
        // A future provider could add a status string we don't know;
        // dropping the row keeps the rest of the list readable rather
        // than failing the whole call.
        let pool = open_in_memory().unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO user_list_cache (provider, user_id, media_id, mal_id, \
                status, progress, score_x100, updated_at, fetched_at, title) \
             VALUES ('anilist', 'u', 1, NULL, 'planning', 0, NULL, 1, 1, 't1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO user_list_cache (provider, user_id, media_id, mal_id, \
                status, progress, score_x100, updated_at, fetched_at, title) \
             VALUES ('anilist', 'u', 2, NULL, 'someday_maybe', 0, NULL, 1, 1, 't2')",
            [],
        )
        .unwrap();
        // Now drop the conn so the read path's pool.get() can acquire
        // (max_size=1 for in-memory pools).
        drop(conn);
        let got = list_entries(&pool, ProviderKind::AniList, "u").unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].media_id.0, 1);
    }
}
