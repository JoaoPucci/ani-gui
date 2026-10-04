//! A parent gone while the backend is still starting up ends it then,
//! not once startup is over.
//!
//! Startup can be long: on a first run it creates the metadata cache
//! and runs every migration, measured at about 24 s on a busy hard
//! disk. Nothing the backend runs has started yet, so there is
//! nothing to wind down; waiting for startup to finish would only
//! leave an orphan behind for as long as it takes.
//!
//! The test makes startup slow without a hook in the backend: it holds
//! an exclusive lock on the cache database, so the backend's first
//! migration waits on SQLite's busy timeout. That is rusqlite's
//! default, 5 s, which has to outlast the second the test waits before
//! closing the pipe plus the 3 s it allows for the exit; without the
//! fix the backend sits there until the timeout fails its startup.
//!
//! Linux only, because the test locks the database at a path it spells
//! out — `<home>/cache/ani-gui-dev/metadata.sqlite`, the Linux layout
//! under the `XDG_CACHE_HOME` that `common::command` sets. macOS keeps
//! its caches elsewhere under the home, and Windows resolves the
//! directory from the known-folder API, so a run there would lock the
//! real dev profile's database. The watch's behaviour during startup
//! is covered on every platform by the unit tests in
//! `src/parent_watch_test.rs`.

#![cfg(target_os = "linux")]

mod common;

use std::time::Duration;

#[test]
fn a_parent_gone_during_startup_ends_the_backend_at_once() {
    let home = tempfile::tempdir().expect("tempdir");
    // `ANI_GUI_DEV` pins the profile directory whatever the build.
    let cache = home.path().join("cache").join("ani-gui-dev");
    std::fs::create_dir_all(&cache).expect("cache dir");
    let lock = rusqlite::Connection::open(cache.join("metadata.sqlite")).expect("open cache");
    lock.execute_batch("BEGIN EXCLUSIVE;").expect("lock cache");

    // The backend logs to its standard output, beside the handshake.
    let log = home.path().join("backend.stdout");
    let mut cmd = common::command(
        std::path::Path::new(env!("CARGO_BIN_EXE_ani-gui-backend")),
        home.path(),
    );
    cmd.env("ANI_GUI_PARENT_STDIN", "1")
        .env("ANI_GUI_DEV", "1")
        .stdout(std::fs::File::create(&log).expect("stdout file"));
    let mut child = cmd.spawn().expect("spawn backend");

    std::thread::sleep(Duration::from_secs(1));
    let still_starting = child.try_wait().expect("try_wait").is_none();
    drop(child.stdin.take());
    let status = common::exited_within(&mut child, Duration::from_secs(3));
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    drop(lock);
    let said = std::fs::read_to_string(&log).unwrap_or_default();

    assert!(still_starting, "the backend was still starting up:\n{said}");
    let status = status
        .unwrap_or_else(|| panic!("a backend whose parent is gone exits during startup:\n{said}"));
    assert!(
        status.success(),
        "nothing failed; it ends cleanly: {status:?}\n{said}"
    );
    assert!(
        said.contains("parent gone during startup"),
        "it says why it ended:\n{said}"
    );
}
