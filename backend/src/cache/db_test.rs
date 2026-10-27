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
    // user_list_cache holds the per-provider list snapshot used
    // by the home Watch Later rail and write-back optimistic
    // state. Pin the column shape here so a future migration
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

/// Before plays marked the mappings they stored, a mapping written
/// within seconds after the show's watch stamp was read as the play's.
/// Opening a cache an earlier build left marks each mapping that rule
/// read as played, once, so a play stored before the upgrade keeps its
/// standing; a mapping the rule read as a guess gets no mark, and
/// neither does one written after.
///
/// It applies no schema migration: refinery aborts on an applied
/// migration it does not have, so one would stop an earlier build from
/// opening the cache after a downgrade.
#[test]
fn opening_an_earlier_cache_marks_the_mappings_the_stamp_rule_read_as_played() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let now = now_secs();
    let ttl = 30 * 24 * 3600;
    let put = |conn: &rusqlite::Connection, key: &str, body: &str, at: i64| {
        conn.execute(
            "INSERT OR REPLACE INTO meta_cache(key, body, fetched_at, ttl_seconds) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![key, body, at, ttl],
        )
        .unwrap();
    };
    let history = |conn: &rusqlite::Connection| -> Vec<i64> {
        conn.prepare("SELECT version FROM refinery_schema_history ORDER BY version")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap()
    };
    // The cache as an earlier build left it: its two migrations, and
    // the rows its plays and guesses wrote.
    let earlier = {
        let mut conn = rusqlite::Connection::open(&path).unwrap();
        crate::cache::schema::run_migrations_to(&mut conn, 2).unwrap();
        // Stamped, then mapped two seconds later: a play's.
        put(
            &conn,
            "watched-at:v1:played",
            &((now - 60) * 1000).to_string(),
            now - 60,
        );
        put(&conn, "allmanga2kitsu:v3:played", "21", now - 58);
        // Mapped in the stamp's own second, the cache reading it first.
        put(
            &conn,
            "watched-at:v1:same",
            &((now - 60) * 1000 + 400).to_string(),
            now - 60,
        );
        put(&conn, "allmanga2kitsu:v3:same", "22", now - 60);
        // Mapped an hour after the stamp: a guess.
        put(
            &conn,
            "watched-at:v1:guess",
            &((now - 3_600) * 1000).to_string(),
            now - 3_600,
        );
        put(&conn, "allmanga2kitsu:v3:guess", "12", now);
        // Mapped with no stamp at all: a guess.
        put(&conn, "allmanga2kitsu:v3:unstamped", "13", now);
        history(&conn)
    };

    let mark = |pool: &SqlitePool, id: &str| {
        meta_cache_get(pool, &format!("allmanga2kitsu:played:v1:{id}")).unwrap()
    };
    let pool = open_pool(&path).unwrap();
    assert_eq!(mark(&pool, "played").as_deref(), Some("21"));
    assert_eq!(mark(&pool, "same").as_deref(), Some("22"));
    assert_eq!(mark(&pool, "guess"), None);
    assert_eq!(mark(&pool, "unstamped"), None);
    assert_eq!(
        history(&pool.get().unwrap()),
        earlier,
        "no migration an earlier build lacks"
    );

    // Once: a mapping written beside a stamp after the upgrade is not
    // marked by opening the cache again.
    {
        let conn = pool.get().unwrap();
        put(&conn, "watched-at:v1:later", &(now * 1000).to_string(), now);
        put(&conn, "allmanga2kitsu:v3:later", "14", now);
    }
    drop(pool);
    let pool = open_pool(&path).unwrap();
    assert_eq!(mark(&pool, "later"), None);
}

/// Live threads whose name starts with `prefix`, read from procfs.
#[cfg(target_os = "linux")]
fn threads_named(prefix: &str) -> usize {
    std::fs::read_dir("/proc/self/task")
        .expect("task list")
        .filter_map(|task| std::fs::read_to_string(task.ok()?.path().join("comm")).ok())
        .filter(|name| name.starts_with(prefix))
        .count()
}

/// A pool's background work runs on helper threads. If every pool had
/// its own, a dropped pool's helpers would linger until its next
/// scheduled cleanup — half a minute later — and a process that opens
/// many pools, as the test binary does, would run into the system's
/// per-process thread limit. Pools share one small set of helpers, so
/// opening and dropping pools leaves no helper threads behind.
///
/// Linux only: the thread names come from procfs.
#[cfg(target_os = "linux")]
#[test]
fn dropped_pools_leave_no_helper_threads_behind() {
    for _ in 0..20 {
        drop(open_in_memory().expect("pool opens"));
    }
    let lingering = threads_named("r2d2-worker");
    assert_eq!(lingering, 0, "{lingering} per-pool helper threads alive");
}

/// r2d2 retires idle and old connections from a job it schedules every
/// 30 seconds for each pool. On the shared helpers that job would
/// outlive its pool and keep running for the life of the process, once
/// for every pool ever opened. A pool here has no such job: a local
/// SQLite connection does not go stale, and retiring the one connection
/// of an in-memory pool would throw its database away.
#[test]
fn pools_schedule_no_connection_reaper() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let file = open_pool(&tmp.path().join("meta.db")).expect("file pool");
    let memory = open_in_memory().expect("memory pool");
    for (name, pool) in [("file", &file), ("memory", &memory)] {
        assert_eq!(
            pool.idle_timeout(),
            None,
            "{name} pool retires idle connections"
        );
        assert_eq!(
            pool.max_lifetime(),
            None,
            "{name} pool retires old connections"
        );
    }
}

/// Two backends can open one cache at once: the integration tests on
/// Windows share the dev profile's directories and start backends in
/// parallel, and a backend run by hand opens the cache the app's own
/// backend has open. Whichever reaches a fresh file
/// first creates and migrates it; the others must wait for that and
/// open it, not fail the start.
#[test]
fn concurrent_opens_of_a_fresh_cache_all_succeed() {
    const OPENERS: usize = 6;
    const ROUNDS: usize = 20;
    for round in 0..ROUNDS {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("metadata.db");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(OPENERS));
        let openers: Vec<_> = (0..OPENERS)
            .map(|_| {
                let path = path.clone();
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    open_pool(&path).map(|_| ())
                })
            })
            .collect();
        let results: Vec<_> = openers.into_iter().map(|t| t.join().unwrap()).collect();
        assert!(
            results.iter().all(Result::is_ok),
            "round {round}: every opener gets the cache: {results:?}"
        );
    }
}

/// Detail rows earlier builds kept with no cover and no banner may have
/// been kept after a banner lookup that got no answer, read then as "no
/// banner" and kept for up to a week. Nothing in a row tells that from
/// AniList answering that it has none, so opening the cache drops every
/// such row once, and each of those shows' detail is fetched again; the
/// rows with a cover or a banner, and every other row, stay. The detail
/// key itself does not move, so removing a show from history still
/// finds its row by prefix.
#[test]
fn detail_rows_with_no_banner_are_dropped_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let now = now_secs();
    let put = |conn: &rusqlite::Connection, key: &str, body: &str| {
        conn.execute(
            "INSERT INTO meta_cache(key, body, fetched_at, ttl_seconds) VALUES (?1, ?2, ?3, 604800)",
            rusqlite::params![key, body, now],
        )
        .unwrap();
    };
    let detail = |cover: &str, banner: &str| {
        format!(r#"{{"id":"1","canonical_title":"x","cover_image":{cover}{banner}}}"#)
    };
    let bannerless = crate::commands::kitsu::anime_detail_key("1");
    let covered = crate::commands::kitsu::anime_detail_key("2");
    let backfilled = crate::commands::kitsu::anime_detail_key("3");
    {
        // The cache as the build before this one left it: migrated, and
        // its earlier plays already marked.
        let mut conn = rusqlite::Connection::open(&path).unwrap();
        crate::cache::schema::run_migrations(&mut conn).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        put(&conn, &bannerless, &detail("null", ""));
        put(
            &conn,
            &covered,
            &detail(r#"{"large":"https://k/c.jpg"}"#, ""),
        );
        put(
            &conn,
            &backfilled,
            &detail("null", r#","anilist_banner":"https://a/b.jpg""#),
        );
        put(&conn, "kitsu:relations:v2:1", r#"{"cover_image":null}"#);
    }
    let pool = open_pool(&path).unwrap();
    let kept = |key: &str| meta_cache_get(&pool, key).unwrap().is_some();
    assert!(!kept(&bannerless), "a detail row with no banner is dropped");
    assert!(kept(&covered) && kept(&backfilled), "rows with one stay");
    assert!(kept("kitsu:relations:v2:1"), "other rows stay");
    // Once: a banner-less row written after the upgrade is the new
    // build's own, kept only on an answer, and stays.
    meta_cache_put(&pool, &bannerless, &detail("null", ""), 604_800).unwrap();
    drop(pool);
    let pool = open_pool(&path).unwrap();
    assert!(meta_cache_get(&pool, &bannerless).unwrap().is_some());
}

/// A cache that has had neither one-time step (user_version 0) gets
/// both in its one pass: the banner-less detail row goes, and a body
/// under the detail prefix that is not JSON is left alone rather than
/// failing the open.
#[test]
fn a_cache_with_neither_step_gets_both_and_tolerates_a_body_that_is_not_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let now = now_secs();
    let bannerless = crate::commands::kitsu::anime_detail_key("1");
    let garbled = crate::commands::kitsu::anime_detail_key("2");
    {
        let mut conn = rusqlite::Connection::open(&path).unwrap();
        crate::cache::schema::run_migrations(&mut conn).unwrap();
        for (key, body) in [
            (bannerless.as_str(), r#"{"id":"1","cover_image":null}"#),
            (garbled.as_str(), "not json at all"),
        ] {
            conn.execute(
                "INSERT INTO meta_cache(key, body, fetched_at, ttl_seconds) VALUES (?1, ?2, ?3, 604800)",
                rusqlite::params![key, body, now],
            )
            .unwrap();
        }
    }
    let pool = open_pool(&path).expect("the cache opens");
    assert!(meta_cache_get(&pool, &bannerless).unwrap().is_none());
    assert!(meta_cache_get(&pool, &garbled).unwrap().is_some());
    let version: i64 = pool
        .get()
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 2);
}

/// The file change counter in a SQLite database's header (bytes 24 to
/// 27, big-endian): one more for every transaction that wrote to the
/// file, in the rollback-journal mode the cache runs in.
fn commits_to(path: &std::path::Path) -> u32 {
    let header = std::fs::read(path).expect("read the database");
    u32::from_be_bytes(header[24..28].try_into().expect("a header"))
}

/// Every write transaction costs a rollback-journal commit four
/// fsyncs: the journal twice, its directory and the database. On a
/// busy or spinning disk each can take a second or more, and a fresh
/// cache is made before the backend's handshake, so the first launch
/// waits on every one of them. A fresh cache is made in three: the
/// migrations' history table, every migration together, and the mark
/// of the plays earlier builds stored.
#[test]
fn a_fresh_cache_is_made_in_three_commits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    drop(open_pool(&path).expect("a fresh cache opens"));
    // The counter counts commits in rollback-journal mode only; in WAL
    // mode it would not move per commit and the count would mislead.
    let mode: String = rusqlite::Connection::open(&path)
        .unwrap()
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "delete", "the cache runs in rollback-journal mode");
    assert_eq!(commits_to(&path), 3);
}

/// A cache an earlier build left with some migrations applied gets the
/// rest, and only the rest: the batch refinery groups is what is
/// pending.
#[test]
fn a_cache_with_some_migrations_applied_gets_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    {
        let mut conn = rusqlite::Connection::open(&path).unwrap();
        crate::cache::schema::run_migrations_to(&mut conn, 1).unwrap();
        conn.execute(
            "INSERT INTO meta_cache(key, body, fetched_at, ttl_seconds) VALUES ('kept', 'v', ?1, 3600)",
            [now_secs()],
        )
        .unwrap();
    }
    let pool = open_pool(&path).expect("the cache upgrades");
    let conn = pool.get().unwrap();
    let applied: Vec<i64> = conn
        .prepare("SELECT version FROM refinery_schema_history ORDER BY version")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();
    assert_eq!(applied, vec![1, 2]);
    drop(conn);
    assert_eq!(meta_cache_get(&pool, "kept").unwrap().as_deref(), Some("v"));
    let pool_conn = pool.get().unwrap();
    pool_conn
        .prepare("SELECT 1 FROM user_list_cache LIMIT 1")
        .expect("V002's table is there");
}

/// The migrations a cache has yet to apply land together or not at
/// all: one that fails leaves none of the others applied, so a cache
/// never stops part-way between two schemas. Here a fresh file already
/// holds a table the second migration creates, and the first
/// migration's tables are not left behind when the second fails.
#[test]
fn a_migration_that_fails_leaves_none_of_its_batch_applied() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch("CREATE TABLE user_list_cache (x INTEGER);")
        .unwrap();
    assert!(open_pool(&path).is_err(), "the second migration fails");
    let conn = rusqlite::Connection::open(&path).unwrap();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();
    assert_eq!(tables, vec!["refinery_schema_history", "user_list_cache"]);
    let applied: i64 = conn
        .query_row("SELECT COUNT(*) FROM refinery_schema_history", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(applied, 0, "no migration recorded as applied");
}

/// The one-time pass runs inside its own single write and only once:
/// on a fresh cache it is the third of the three commits, and opening
/// the cache again writes nothing.
#[test]
fn a_fresh_cache_runs_the_one_time_pass_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    drop(open_pool(&path).expect("a fresh cache opens"));
    let version: i64 = rusqlite::Connection::open(&path)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 2, "both one-time steps are recorded");
    let after_first = commits_to(&path);
    drop(open_pool(&path).expect("the cache opens again"));
    assert_eq!(
        commits_to(&path),
        after_first,
        "a second open writes nothing"
    );
}

/// A cache an earlier build left with its plays marked (user_version 1)
/// gets the banner step alone, in one write, and the next open writes
/// nothing.
#[test]
fn a_cache_at_version_one_drops_its_bannerless_rows_in_one_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let bannerless = crate::commands::kitsu::anime_detail_key("1");
    drop(open_pool(&path).expect("a fresh cache opens"));
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute(
            "INSERT INTO meta_cache(key, body, fetched_at, ttl_seconds) VALUES (?1, ?2, ?3, 604800)",
            rusqlite::params![bannerless, r#"{"id":"1","cover_image":null}"#, now_secs()],
        )
        .unwrap();
    }
    let before = commits_to(&path);
    let pool = open_pool(&path).expect("the cache opens");
    assert!(meta_cache_get(&pool, &bannerless).unwrap().is_none());
    drop(pool);
    assert_eq!(
        commits_to(&path),
        before + 1,
        "the step and its gate are one write"
    );
    let again = commits_to(&path);
    drop(open_pool(&path).expect("the cache opens again"));
    assert_eq!(commits_to(&path), again, "and it does not run again");
}

/// A cache already at version 2 is not swept: a banner-less row an
/// earlier build wrote after it opened the cache stays, and opening it
/// writes nothing.
#[test]
fn a_cache_at_version_two_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let bannerless = crate::commands::kitsu::anime_detail_key("1");
    drop(open_pool(&path).expect("a fresh cache opens"));
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute(
            "INSERT INTO meta_cache(key, body, fetched_at, ttl_seconds) VALUES (?1, ?2, ?3, 604800)",
            rusqlite::params![bannerless, r#"{"id":"1","cover_image":null}"#, now_secs()],
        )
        .unwrap();
    }
    let before = commits_to(&path);
    let pool = open_pool(&path).expect("the cache opens");
    assert!(meta_cache_get(&pool, &bannerless).unwrap().is_some());
    drop(pool);
    assert_eq!(commits_to(&path), before);
}
