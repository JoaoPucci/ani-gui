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
//! On Linux and macOS the backend's data lands in a temporary HOME.
//! On Windows the platform directories come from the known-folder
//! API rather than the environment, so the run uses the dev
//! profile's directories (`ani-gui-dev`, as a debug build always
//! does) — the same ones `pnpm dev` uses.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn spawn_backend(home: &std::path::Path, watch_parent: bool) -> Child {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ani-gui-backend"));
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("HOME", home)
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env_remove("ANI_GUI_PARENT_STDIN");
    if watch_parent {
        cmd.env("ANI_GUI_PARENT_STDIN", "1");
    }
    let mut child = cmd.spawn().expect("spawn backend");

    // Wait for the handshake, so the watch is judged on a backend that
    // is up and serving, not one still starting.
    let stdout = child.stdout.take().expect("stdout");
    let mut lines = BufReader::new(stdout).lines();
    let mut seen = (false, false);
    while !(seen.0 && seen.1) {
        let line = lines
            .next()
            .expect("backend closed stdout before the handshake")
            .expect("read stdout");
        seen.0 |= line.starts_with("ANI_GUI_LISTENING ");
        seen.1 |= line.starts_with("ANI_GUI_INTERNAL_SECRET ");
    }
    // Keep draining so a chatty backend never blocks on a full pipe.
    std::thread::spawn(move || for _ in lines {});
    child
}

fn exited_within(child: &mut Child, wait: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

// One test, two cases in turn: on Windows both backends would share
// the dev profile's SQLite database, and two first opens racing their
// migrations is a different test.
#[test]
fn the_backend_ends_with_its_parent_and_only_when_asked_to() {
    let home = tempfile::tempdir().expect("tempdir");

    let mut watched = spawn_backend(home.path(), true);
    drop(watched.stdin.take());
    let status = exited_within(&mut watched, Duration::from_secs(10));
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
    let status = exited_within(&mut unwatched, Duration::from_millis(1_500));
    let _ = unwatched.kill();
    let _ = unwatched.wait();
    assert!(
        status.is_none(),
        "a backend run by hand keeps running when its stdin closes: {status:?}"
    );
}
