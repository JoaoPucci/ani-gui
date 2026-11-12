//! Tests for `crate::cache::sqlite_manager`. Extracted via `#[path]` so
//! the test bodies don't count toward the module's CCN.

use super::*;
use r2d2::ManageConnection;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

#[test]
fn a_file_manager_creates_and_opens_the_file_at_its_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let manager = SqliteConnectionManager::file(&path);
    let conn = manager.connect().expect("the file opens");
    conn.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (1);")
        .unwrap();
    drop(conn);
    assert!(path.exists(), "the database is the file at the path");
    let reopened = Connection::open(&path).unwrap();
    assert_eq!(count(&reopened, "t"), 1);
}

#[test]
fn connections_of_one_memory_manager_share_its_database() {
    let manager = SqliteConnectionManager::memory();
    let first = manager.connect().expect("an in-memory connection");
    first
        .execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (1);")
        .unwrap();
    drop(first);
    // The first connection is gone, and the database outlives it: the
    // manager keeps it until the manager itself is dropped.
    let second = manager.connect().expect("a second connection");
    assert_eq!(count(&second, "t"), 1);
}

#[test]
fn two_memory_managers_never_share_a_database() {
    let a = SqliteConnectionManager::memory();
    let b = SqliteConnectionManager::memory();
    a.connect()
        .unwrap()
        .execute_batch("CREATE TABLE only_in_a(x);")
        .unwrap();
    let in_b: i64 = b
        .connect()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'only_in_a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(in_b, 0);
}

#[test]
fn a_file_manager_opens_read_write_and_creates_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fresh.db");
    let conn = SqliteConnectionManager::file(&path).connect().unwrap();
    assert!(!conn.is_readonly(rusqlite::MAIN_DB).unwrap());
    assert!(path.exists());
}

#[test]
fn the_flags_given_are_the_flags_a_connection_opens_with() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    Connection::open(&path)
        .unwrap()
        .execute_batch("CREATE TABLE t(x);")
        .unwrap();
    let manager = SqliteConnectionManager::file(&path)
        .with_flags(OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX);
    let conn = manager.connect().expect("a read-only connection");
    assert!(conn.is_readonly(rusqlite::MAIN_DB).unwrap());
    assert!(conn.execute("INSERT INTO t VALUES (1)", []).is_err());
}

#[test]
fn without_the_create_flag_a_missing_file_is_not_made() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.db");
    // The same flags open the file once it exists: the refusal is the
    // missing create flag's.
    let present = dir.path().join("present.db");
    Connection::open(&present).unwrap();
    assert!(SqliteConnectionManager::file(&present)
        .with_flags(OpenFlags::SQLITE_OPEN_READ_WRITE)
        .connect()
        .is_ok());
    let manager =
        SqliteConnectionManager::file(&path).with_flags(OpenFlags::SQLITE_OPEN_READ_WRITE);
    assert!(manager.connect().is_err());
    assert!(!path.exists());
}

#[test]
fn init_runs_on_every_connection_the_manager_opens() {
    let runs = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&runs);
    let manager = SqliteConnectionManager::memory().with_init(move |c| {
        seen.fetch_add(1, Ordering::SeqCst);
        c.busy_timeout(std::time::Duration::from_millis(1234))
    });
    for expected in 1..=3 {
        let conn = manager.connect().unwrap();
        let timeout: i64 = conn
            .pragma_query_value(None, "busy_timeout", |r| r.get(0))
            .unwrap();
        assert_eq!(timeout, 1234, "init set the pragma on this connection");
        assert_eq!(runs.load(Ordering::SeqCst), expected);
    }
}

#[test]
fn an_init_that_fails_fails_the_connection() {
    // The same init, failing only when told to: the refusal is the
    // init's, not a manager that opens nothing.
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let told = Arc::clone(&fail);
    let manager = SqliteConnectionManager::memory().with_init(move |c| {
        if told.load(Ordering::SeqCst) {
            c.execute_batch("THIS IS NOT SQL")
        } else {
            Ok(())
        }
    });
    assert!(manager.connect().is_ok());
    fail.store(true, Ordering::SeqCst);
    assert!(manager.connect().is_err());
}

#[test]
fn an_open_connection_is_valid_and_not_broken() {
    let manager = SqliteConnectionManager::memory();
    let mut conn = manager.connect().unwrap();
    assert!(manager.is_valid(&mut conn).is_ok());
    assert!(!manager.has_broken(&mut conn));
}

#[test]
fn a_pool_over_the_manager_hands_out_working_connections() {
    let dir = tempfile::tempdir().unwrap();
    // Helpers of its own, under a name of its own: r2d2's default set
    // is named `r2d2-worker`, which a test of the cache's pools counts
    // as left behind by them.
    let helpers = scheduled_thread_pool::ScheduledThreadPool::builder()
        .num_threads(1)
        .thread_name_pattern("manager-test-{}")
        .build();
    let pool = r2d2::Pool::builder()
        .thread_pool(Arc::new(helpers))
        .max_size(2)
        .build(SqliteConnectionManager::file(dir.path().join("pool.db")))
        .expect("the pool builds");
    pool.get()
        .unwrap()
        .execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (7);")
        .unwrap();
    let a = pool.get().unwrap();
    let b = pool.get().unwrap();
    assert_eq!(count(&a, "t"), 1);
    assert_eq!(count(&b, "t"), 1);
}

#[test]
fn the_debug_form_names_the_source_and_stands_in_for_the_init() {
    let shown = format!(
        "{:?}",
        SqliteConnectionManager::file("/x/metadata.db").with_init(|_| Ok(()))
    );
    assert!(shown.contains("SqliteConnectionManager"), "{shown}");
    assert!(shown.contains("metadata.db"), "{shown}");
    assert!(shown.contains("InitFn"), "{shown}");
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(32))]

    /// Whatever one connection of a memory manager writes, any later
    /// connection of the same manager reads back, in full.
    #[test]
    fn what_one_memory_connection_writes_the_next_reads(
        values in proptest::collection::vec(proptest::prelude::any::<i64>(), 0..20)
    ) {
        let manager = SqliteConnectionManager::memory();
        {
            let writer = manager.connect().unwrap();
            writer.execute_batch("CREATE TABLE t(x INTEGER)").unwrap();
            for v in &values {
                writer.execute("INSERT INTO t VALUES (?1)", [v]).unwrap();
            }
        }
        let reader = manager.connect().unwrap();
        let mut stmt = reader.prepare("SELECT x FROM t ORDER BY rowid").unwrap();
        let read: Vec<i64> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(std::result::Result::unwrap)
            .collect();
        proptest::prop_assert_eq!(read, values);
    }
}
