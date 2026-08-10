//! Tests for `crate::db`. Extracted via `#[path]` so the inline
//! `mod tests { ... }` block doesn't count toward the file's CCN — per
//! `project_crap_inline_test_gotcha`.

use super::*;

#[test]
fn open_in_memory_runs_migrations_and_creates_tables() {
    let pool = open_in_memory().expect("pool opens");
    let conn = pool.get().expect("checkout");
    // Migrations should have created all four tables (V001 created the
    // first three; V002 added user_list_cache for account integration).
    let tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'refinery%' \
                 ORDER BY name",
        )
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(std::result::Result::unwrap)
        .collect();
    assert_eq!(
        tables,
        vec![
            "image_index",
            "meta_cache",
            "title_match",
            "user_list_cache"
        ]
    );
}

#[test]
fn meta_cache_round_trips_within_ttl() {
    let pool = open_in_memory().unwrap();
    meta_cache_put(&pool, "k", "{\"v\":1}", 60).unwrap();
    assert_eq!(
        meta_cache_get(&pool, "k").unwrap().as_deref(),
        Some("{\"v\":1}")
    );
}

#[test]
fn meta_cache_returns_none_for_missing_keys() {
    let pool = open_in_memory().unwrap();
    assert_eq!(meta_cache_get(&pool, "missing").unwrap(), None);
}

#[test]
fn meta_cache_returns_none_when_entry_is_past_ttl() {
    // ttl=0 means "expired immediately" per the `>=` semantics in
    // meta_cache_get; no sleep required and no raw-conn manipulation.
    let pool = open_in_memory().unwrap();
    meta_cache_put(&pool, "stale", "v", 0).unwrap();
    assert_eq!(meta_cache_get(&pool, "stale").unwrap(), None);
}

#[test]
fn meta_cache_put_replaces_existing_value() {
    let pool = open_in_memory().unwrap();
    meta_cache_put(&pool, "k", "first", 60).unwrap();
    meta_cache_put(&pool, "k", "second", 60).unwrap();
    assert_eq!(
        meta_cache_get(&pool, "k").unwrap().as_deref(),
        Some("second")
    );
}

#[test]
fn meta_cache_clear_removes_all_rows() {
    let pool = open_in_memory().unwrap();
    meta_cache_put(&pool, "a", "1", 60).unwrap();
    meta_cache_put(&pool, "b", "2", 60).unwrap();
    meta_cache_clear(&pool).unwrap();
    assert_eq!(meta_cache_get(&pool, "a").unwrap(), None);
    assert_eq!(meta_cache_get(&pool, "b").unwrap(), None);
}

#[test]
fn title_match_round_trips_with_both_ids() {
    let pool = open_in_memory().unwrap();
    title_match_put(&pool, "one piece", Some("12"), Some(21)).unwrap();
    let row = title_match_get(&pool, "one piece")
        .unwrap()
        .expect("present");
    assert_eq!(row.kitsu_id.as_deref(), Some("12"));
    assert_eq!(row.anilist_id, Some(21));
    assert!(row.fetched_at > 0);
}

#[test]
fn title_match_round_trips_with_only_one_id() {
    let pool = open_in_memory().unwrap();
    title_match_put(&pool, "obscure", Some("999"), None).unwrap();
    let row = title_match_get(&pool, "obscure").unwrap().expect("present");
    assert_eq!(row.kitsu_id.as_deref(), Some("999"));
    assert_eq!(row.anilist_id, None);
}

#[test]
fn title_match_get_returns_none_for_missing_query() {
    let pool = open_in_memory().unwrap();
    assert!(title_match_get(&pool, "never seen").unwrap().is_none());
}

#[test]
fn title_match_put_is_upsert() {
    let pool = open_in_memory().unwrap();
    title_match_put(&pool, "k", Some("first"), None).unwrap();
    title_match_put(&pool, "k", Some("second"), Some(42)).unwrap();
    let row = title_match_get(&pool, "k").unwrap().unwrap();
    assert_eq!(row.kitsu_id.as_deref(), Some("second"));
    assert_eq!(row.anilist_id, Some(42));
}

#[test]
fn migrations_are_idempotent_across_pool_opens() {
    // Opening a second pool against the same in-memory DB would be
    // a fresh DB (max_size=1 + :memory: per-pool), so this test
    // instead verifies that calling the migration runner twice on
    // the same connection is a no-op.
    let pool = open_in_memory().unwrap();
    let mut conn = pool.get().unwrap();
    run_migrations(&mut conn).expect("second run is a no-op");
    run_migrations(&mut conn).expect("third run is a no-op");
}

#[test]
fn v002_creates_user_list_cache_with_expected_columns() {
    // PR #1 of the account integration chain. user_list_cache
    // holds the per-provider list snapshot used by the home
    // Watch Later rail (PR #2) and write-back optimistic state
    // (PR #4). Pin the column shape here so a future migration
    // can't silently drop a column the cache reader depends on.
    let pool = open_in_memory().unwrap();
    let conn = pool.get().unwrap();
    let cols: Vec<(String, String)> = conn
        .prepare("PRAGMA table_info(user_list_cache)")
        .unwrap()
        .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
        .unwrap()
        .map(std::result::Result::unwrap)
        .collect();
    let names: Vec<&str> = cols.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        names.contains(&"provider"),
        "missing provider column: {names:?}"
    );
    assert!(names.contains(&"user_id"), "missing user_id: {names:?}");
    assert!(names.contains(&"media_id"), "missing media_id: {names:?}");
    assert!(names.contains(&"mal_id"), "missing mal_id: {names:?}");
    assert!(names.contains(&"status"), "missing status: {names:?}");
    assert!(names.contains(&"progress"), "missing progress: {names:?}");
    assert!(
        names.contains(&"score_x100"),
        "missing score_x100: {names:?}"
    );
    assert!(
        names.contains(&"updated_at"),
        "missing updated_at: {names:?}"
    );
    assert!(
        names.contains(&"fetched_at"),
        "missing fetched_at: {names:?}"
    );
    assert!(names.contains(&"title"), "missing title: {names:?}");
}

#[test]
fn v002_user_list_cache_round_trip_insert_select() {
    // Sanity check that the schema actually stores + retrieves a
    // representative row. Caught a bug in an earlier draft where
    // the PRIMARY KEY column order was wrong.
    let pool = open_in_memory().unwrap();
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO user_list_cache (\
                provider, user_id, media_id, mal_id, status, progress, \
                score_x100, updated_at, fetched_at, title) \
             VALUES ('anilist', '12345', 16498, 16498, 'planning', 0, \
                NULL, 1700000000, 1700000005, 'Shingeki no Kyojin')",
        [],
    )
    .unwrap();
    let row: (String, String, i64, Option<i64>, String, i64) = conn
        .query_row(
            "SELECT provider, user_id, media_id, mal_id, status, progress \
                 FROM user_list_cache WHERE media_id = 16498",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(row.0, "anilist");
    assert_eq!(row.1, "12345");
    assert_eq!(row.2, 16498);
    assert_eq!(row.3, Some(16498));
    assert_eq!(row.4, "planning");
    assert_eq!(row.5, 0);
}
