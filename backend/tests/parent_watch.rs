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
//!
//! A parent that dies takes the other pipes with it as well, so the
//! backend's own output has nowhere to go from that moment; the last
//! two cases hold it to stopping regardless.

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

// One test, its cases in turn: on Windows the backends would share
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

    a_dead_parent_reads_nothing(home.path());
    a_parent_dead_before_the_handshake(home.path());
}

/// What the backend printed to the standard error this test gave it.
fn said(log: &std::path::Path) -> String {
    std::fs::read_to_string(log).unwrap_or_default()
}

/// A parent that dies takes every pipe with it, not just stdin: nothing
/// reads the backend's output any more, and its next log line cannot
/// be written. That has to be a line lost and nothing else. The
/// logger's answer to a failed write is to complain on standard
/// error, and with standard error gone too the complaint itself
/// fails — as a panic, which in the shipped build (`panic = "abort"`)
/// kills the backend on the spot, before it has stopped anything it
/// started.
///
/// Standard error goes to a file here so the complaint can be seen
/// instead of becoming that panic: a backend that stops without a
/// word never reaches it.
fn a_dead_parent_reads_nothing(home: &std::path::Path) {
    let log = home.join("dead-parent.stderr");
    let mut cmd = common::command(
        std::path::Path::new(env!("CARGO_BIN_EXE_ani-gui-backend")),
        home,
    );
    cmd.env("ANI_GUI_PARENT_STDIN", "1")
        .stderr(std::fs::File::create(&log).expect("stderr file"));
    let (mut backend, output) = common::start_keeping_output(cmd);

    drop(output);
    drop(backend.child.stdin.take());
    let status = common::exited_within(&mut backend.child, Duration::from_secs(10));
    if status.is_none() {
        let _ = backend.child.kill();
    }
    let status = status.expect("the backend exits once its parent is gone");
    let said = said(&log);
    assert!(
        status.success(),
        "losing its output does not turn the stop into a failure: {status:?}\n{said}"
    );
    assert!(
        !said.contains("Unable to write") && !said.contains("panicked"),
        "a log line with nowhere to go is dropped, not complained about:\n{said}"
    );
}

/// The same, earlier: a parent gone before the backend has announced
/// itself. The handshake cannot be delivered, so the backend has no
/// reason to run — but it ends as a failure it reports with its exit
/// status, not as a panic.
fn a_parent_dead_before_the_handshake(home: &std::path::Path) {
    let log = home.join("no-handshake.stderr");
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    let mut cmd = common::command(
        std::path::Path::new(env!("CARGO_BIN_EXE_ani-gui-backend")),
        home,
    );
    cmd.env("ANI_GUI_PARENT_STDIN", "1")
        .stdout(writer)
        .stderr(std::fs::File::create(&log).expect("stderr file"));
    let mut child = cmd.spawn().expect("spawn backend");

    let status = common::exited_within(&mut child, Duration::from_secs(30));
    if status.is_none() {
        let _ = child.kill();
    }
    let status = status.expect("a backend that cannot announce itself exits");
    let said = said(&log);
    assert!(
        !status.success(),
        "an undelivered handshake is a failed start: {status:?}"
    );
    assert!(
        !said.contains("Unable to write") && !said.contains("panicked"),
        "it fails by its exit status, not by a panic:\n{said}"
    );
}
