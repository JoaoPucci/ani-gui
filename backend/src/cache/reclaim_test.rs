//! The launch reclaim over a cache on disk and in memory.

use super::*;
use crate::cache::families::live;

fn conn_with(rows: &[(&str, i64, i64)]) -> Connection {
    let conn = Connection::open_in_memory().expect("open");
    conn.execute_batch(
        "CREATE TABLE meta_cache (key TEXT PRIMARY KEY, body TEXT NOT NULL, \
         fetched_at INTEGER NOT NULL, ttl_seconds INTEGER NOT NULL)",
    )
    .expect("schema");
    let now = crate::cache::db::now_secs();
    for (key, age, ttl) in rows {
        conn.execute(
            "INSERT INTO meta_cache VALUES (?1, 'b', ?2, ?3)",
            rusqlite::params![key, now - age, ttl],
        )
        .expect("row");
    }
    conn
}

fn keys(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT key FROM meta_cache ORDER BY key")
        .expect("prep");
    stmt.query_map([], |r| r.get(0))
        .expect("query")
        .collect::<std::result::Result<_, _>>()
        .expect("rows")
}

const DAY: i64 = 86_400;

#[test]
fn rows_no_reader_asks_for_go_and_the_rest_stay() {
    let fresh_detail = crate::commands::kitsu::anime_detail_key("12");
    let expired_detail = crate::commands::kitsu::anime_detail_key("13");
    let expired_mapping = crate::commands::kitsu::allmanga_kitsu_key("hianime:x-1");
    let fresh_verdict = crate::commands::availability::cache_key("12", "sub");
    let mut conn = conn_with(&[
        (&fresh_detail, 0, DAY),
        (&expired_detail, 2 * DAY, DAY),
        // Read past its lifetime by a history removal: stays.
        (&expired_mapping, 40 * DAY, 30 * DAY),
        (&fresh_verdict, 0, DAY),
        // An earlier build's re-keyed rows, fresh or not.
        ("availability:v13:45254:sub", 0, DAY),
        (
            "allmanga2kitsu:v3:naruto-shippuden-3687",
            40 * DAY,
            30 * DAY,
        ),
        (
            "play:v12:Naruto: Shippuuden:sub:best:366:2007:500:TV",
            50 * DAY,
            7 * DAY,
        ),
        ("title-match:v2:naruto shippuden:c1", 0, 30 * DAY),
    ]);

    let report = reclaim(&mut conn, &live()).expect("reclaim");

    let mut kept = vec![fresh_detail, expired_mapping, fresh_verdict];
    kept.sort();
    assert_eq!(keys(&conn), kept);
    assert_eq!(
        report,
        ReclaimReport {
            undeclared_rows: 4,
            expired_rows: 1,
        }
    );
}

#[test]
fn a_second_reclaim_finds_nothing() {
    let mut conn = conn_with(&[
        ("availability:v13:1:sub", 0, DAY),
        (
            &crate::commands::kitsu::anime_detail_key("13"),
            2 * DAY,
            DAY,
        ),
    ]);
    let _ = reclaim(&mut conn, &live()).expect("first");
    assert_eq!(
        reclaim(&mut conn, &live()).expect("second"),
        ReclaimReport::default()
    );
}

#[test]
fn a_cache_opened_for_the_app_is_reclaimed_under_its_lock() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("metadata.sqlite");
    {
        let pool = crate::cache::open_pool(&path).expect("open");
        crate::cache::families::undeclared_allowed(|| {
            crate::cache::meta_cache_put(&pool, "play:v12:old:sub:best:1:-:-:-", "{}", 3600)
        })
        .expect("seed");
    }

    let (pool, report) = crate::cache::open_pool_reclaiming(&path).expect("reopen");

    assert_eq!(report.undeclared_rows, 1);
    let left = crate::cache::families::undeclared_allowed(|| {
        crate::cache::meta_cache_get(&pool, "play:v12:old:sub:best:1:-:-:-")
    })
    .expect("read");
    assert_eq!(left, None);
}
