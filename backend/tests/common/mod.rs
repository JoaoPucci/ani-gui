//! Shared by the integration tests that run the real backend binary.
//!
//! On Linux and macOS the backend's data lands under the `home` each
//! test passes. On Windows the platform directories come from the
//! known-folder API rather than the environment, so a run there uses
//! the dev profile's directories (`ani-gui-dev`, as a debug build
//! always does) — the same ones `pnpm dev` uses.

// Each test crate uses its own subset.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::{mpsc, Arc, Mutex};
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
    /// The other lanes it announced ahead of that line, by name, each
    /// an origin of its own.
    pub lanes: std::collections::BTreeMap<String, String>,
}

/// A command for the backend at `exe`, its data under `home`, stdin a
/// pipe the test holds the way Electron does, logging at the level it
/// ships with. Whether the backend watches that pipe is the caller's
/// to set. Stderr is a pipe [`start`] drains, so a backend that never
/// completes its handshake reports what it wrote there; a caller that
/// redirects it gives that up.
pub fn command(exe: &Path, home: &Path) -> Command {
    let mut cmd = Command::new(exe);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("HOME", home)
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_DATA_HOME", home.join("data"))
        // Pinned, not inherited: tests that close the backend's output
        // rely on it logging at its default level — quieter and they
        // prove nothing, louder and an unread pipe can fill.
        .env("RUST_LOG", "ani_gui=info")
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
/// before printing both handshake lines — after killing it, with its
/// exit status, its stdout (which carries its log) and its stderr in
/// the message.
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
    let stderr = child.stderr.take().map(drain_stderr);
    // A thread owns the read, because a blocking read has no timeout
    // of its own; it hands the reader back once the handshake is in,
    // or everything it read once the output ends without one.
    type Handshake = (
        String,
        std::collections::BTreeMap<String, String>,
        BufReader<ChildStdout>,
    );
    let (tx, rx) = mpsc::channel::<Result<Handshake, String>>();
    std::thread::spawn(move || {
        let mut output = BufReader::new(stdout);
        let mut api_base = None;
        let mut lanes = std::collections::BTreeMap::new();
        let mut secret_seen = false;
        let mut seen = String::new();
        let mut line = String::new();
        while api_base.is_none() || !secret_seen {
            line.clear();
            match output.read_line(&mut line) {
                Ok(n) if n > 0 => seen.push_str(&line),
                Ok(_) => {
                    let _ = tx.send(Err(seen));
                    return;
                }
                Err(e) => {
                    let _ = tx.send(Err(format!("{seen}<stdout read failed: {e}>")));
                    return;
                }
            }
            if let Some(base) = line.strip_prefix("ANI_GUI_LISTENING ") {
                api_base = Some(base.trim().to_string());
            }
            if let Some(lane) = line.strip_prefix("ANI_GUI_LANE ") {
                if let Some((name, base)) = lane.trim().split_once(' ') {
                    lanes.insert(name.to_string(), base.trim().to_string());
                }
            }
            secret_seen |= line.starts_with("ANI_GUI_INTERNAL_SECRET ");
        }
        let _ = tx.send(Ok((api_base.expect("checked by the loop"), lanes, output)));
    });

    let (api_base, lanes, output) = match rx.recv_timeout(HANDSHAKE_TIMEOUT) {
        Ok(Ok(handshake)) => handshake,
        Ok(Err(stdout)) => fail(&mut child, "its output ended", &stdout, stderr.as_ref()),
        Err(_) => fail(
            &mut child,
            "it stayed silent past the timeout",
            "<still being read>",
            stderr.as_ref(),
        ),
    };
    (
        Backend {
            child,
            api_base,
            lanes,
        },
        output,
    )
}

/// What the backend has written to stderr so far, read on a thread of
/// its own until the pipe closes, so the pipe never fills.
type Captured = Arc<Mutex<Vec<u8>>>;

fn drain_stderr(mut stderr: std::process::ChildStderr) -> Captured {
    let captured: Captured = Arc::default();
    let sink = Arc::clone(&captured);
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = stderr.read(&mut buf) {
            if n == 0 {
                break;
            }
            sink.lock()
                .expect("stderr buffer")
                .extend_from_slice(&buf[..n]);
        }
    });
    captured
}

/// Fail the start with everything the backend said: its exit status
/// (or that it was still running and has been killed), its stdout up
/// to the failure, which carries its log, and its stderr.
fn fail(child: &mut Child, why: &str, stdout: &str, stderr: Option<&Captured>) -> ! {
    let status = match exited_within(child, Duration::from_secs(5)) {
        Some(status) => format!("exited with {status}"),
        None => {
            let _ = child.kill();
            let _ = child.wait();
            "still running, killed".to_string()
        }
    };
    // The child is gone, so its stderr is closing; give the drain a
    // moment to read the last of it.
    std::thread::sleep(Duration::from_millis(200));
    let stderr = stderr.map_or_else(
        || "<redirected by the caller>".to_string(),
        |c| String::from_utf8_lossy(&c.lock().expect("stderr buffer")).into_owned(),
    );
    panic!(
        "the backend did not complete its handshake: {why}; it {status}\n\
         --- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
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
