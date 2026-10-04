//! Shared by the integration tests that run the real backend binary.
//!
//! On Linux and macOS the backend's data lands under the `home` each
//! test passes. On Windows the platform directories come from the
//! known-folder API rather than the environment, so a run there uses
//! the dev profile's directories (`ani-gui-dev`, as a debug build
//! always does) — the same ones `pnpm dev` uses.

// Each test crate uses its own subset.
#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long a backend may take to print its handshake. Generous — a
/// debug build on a loaded runner — but finite: a backend that never
/// prints it must fail the test, not hang the run.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);

/// A running backend, past its handshake.
pub struct Backend {
    pub child: Child,
    /// The origin it printed, `http://127.0.0.1:<port>`.
    pub api_base: String,
}

/// A command for the backend at `exe`, its data under `home`, stdin a
/// pipe the test holds the way Electron does. Whether the backend
/// watches that pipe is the caller's to set.
pub fn command(exe: &Path, home: &Path) -> Command {
    let mut cmd = Command::new(exe);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("HOME", home)
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env_remove("ANI_GUI_PARENT_STDIN");
    cmd
}

/// Spawn `cmd` and wait for the handshake, so whatever the test does
/// next is done to a backend that is up and serving. Its output is
/// drained from then on, so a chatty backend never blocks on a full
/// pipe.
///
/// # Panics
/// When the backend exits, or stays silent past [`HANDSHAKE_TIMEOUT`],
/// before printing both handshake lines — after killing it.
pub fn start(cmd: Command) -> Backend {
    let (backend, output) = start_keeping_output(cmd);
    std::thread::spawn(move || for _ in output.lines() {});
    backend
}

/// [`start`], handing the backend's output back instead of draining
/// it — for a test that wants to stop reading it, as a parent that
/// died does.
///
/// # Panics
/// As [`start`].
pub fn start_keeping_output(mut cmd: Command) -> (Backend, BufReader<ChildStdout>) {
    let mut child = cmd.spawn().expect("spawn backend");
    let stdout = child.stdout.take().expect("stdout");
    // A thread owns the read, because a blocking read has no timeout
    // of its own; it hands the reader back once the handshake is in.
    let (tx, rx) = mpsc::channel::<(String, BufReader<ChildStdout>)>();
    std::thread::spawn(move || {
        let mut output = BufReader::new(stdout);
        let mut api_base = None;
        let mut secret_seen = false;
        let mut line = String::new();
        while api_base.is_none() || !secret_seen {
            line.clear();
            match output.read_line(&mut line) {
                Ok(n) if n > 0 => {}
                _ => return,
            }
            if let Some(base) = line.strip_prefix("ANI_GUI_LISTENING ") {
                api_base = Some(base.trim().to_string());
            }
            secret_seen |= line.starts_with("ANI_GUI_INTERNAL_SECRET ");
        }
        let _ = tx.send((api_base.expect("checked by the loop"), output));
    });

    let Ok((api_base, output)) = rx.recv_timeout(HANDSHAKE_TIMEOUT) else {
        let _ = child.kill();
        let _ = child.wait();
        panic!("the backend did not complete its handshake");
    };
    (Backend { child, api_base }, output)
}

/// The child's exit status, if it exits within `wait`.
pub fn exited_within(child: &mut Child, wait: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}
