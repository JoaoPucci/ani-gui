//! The backend binary ends when the process that spawned it is gone.
//!
//! Electron spawns the backend with a stdin pipe it never writes to
//! or closes, and sets `ANI_GUI_PARENT_STDIN=1`. However Electron
//! dies — a quit, a crash, SIGKILL, `taskkill /F` — the OS closes its
//! end of that pipe, and the backend reads end of file. Dropping the
//! pipe's write end here stands in for that death.
//!
//! Without the variable — a backend run by hand from a terminal —
//! stdin is not watched, so closing it ends nothing.

mod common;

use std::time::Duration;

fn spawn_backend(home: &std::path::Path, watch_parent: bool) -> std::process::Child {
    let mut cmd = common::command(
        std::path::Path::new(env!("CARGO_BIN_EXE_ani-gui-backend")),
        home,
    );
    if watch_parent {
        cmd.env("ANI_GUI_PARENT_STDIN", "1");
    }
    common::start(cmd).child
}

// One test, two cases in turn: on Windows both backends would share
// the dev profile's SQLite database (see `common`), and two first
// opens racing their migrations is a different test.
#[test]
fn the_backend_ends_with_its_parent_and_only_when_asked_to() {
    let home = tempfile::tempdir().expect("tempdir");

    let mut watched = spawn_backend(home.path(), true);
    drop(watched.stdin.take());
    let status = common::exited_within(&mut watched, Duration::from_secs(10));
    if status.is_none() {
        let _ = watched.kill();
    }
    let status = status.expect("a watched backend exits once its parent's pipe closes");
    assert!(
        status.success(),
        "the parent-gone shutdown is a clean exit: {status:?}"
    );

    let mut unwatched = spawn_backend(home.path(), false);
    drop(unwatched.stdin.take());
    let status = common::exited_within(&mut unwatched, Duration::from_millis(1_500));
    let _ = unwatched.kill();
    let _ = unwatched.wait();
    assert!(
        status.is_none(),
        "a backend run by hand keeps running when its stdin closes: {status:?}"
    );
}
