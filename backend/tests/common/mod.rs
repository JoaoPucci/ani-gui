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
/// before printing both handshake lines. The message carries its exit
/// status (or that it was still running, and has been killed), the
/// stdout it wrote until then, which carries its log, and its stderr.
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
    let stderr = child.stderr.take().map(drain);
    // A thread owns the read, because a blocking read has no timeout
    // of its own; it hands the reader back once the handshake is in.
    // What it reads before then is kept where a failed start can
    // report it, whether the output ended or the wait ran out.
    type Handshake = (
        String,
        std::collections::BTreeMap<String, String>,
        BufReader<ChildStdout>,
    );
    let said: Arc<Mutex<String>> = Arc::default();
    let heard = Arc::clone(&said);
    let (tx, rx) = mpsc::channel::<Handshake>();
    std::thread::spawn(move || {
        let mut output = BufReader::new(stdout);
        let mut api_base = None;
        let mut lanes = std::collections::BTreeMap::new();
        let mut secret_seen = false;
        let mut line = String::new();
        while api_base.is_none() || !secret_seen {
            line.clear();
            match output.read_line(&mut line) {
                Ok(n) if n > 0 => heard.lock().expect("stdout so far").push_str(&line),
                Ok(_) => return,
                Err(e) => {
                    let note = format!("<stdout read failed: {e}>");
                    heard.lock().expect("stdout so far").push_str(&note);
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
        let _ = tx.send((api_base.expect("checked by the loop"), lanes, output));
    });

    let (api_base, lanes, output) = match rx.recv_timeout(HANDSHAKE_TIMEOUT) {
        Ok(handshake) => handshake,
        // The reader dropped its sender: the output ended first.
        // Output that ended usually means an exit on its way, so the
        // backend gets a moment to finish it; one still silent past
        // the timeout gets none.
        Err(mpsc::RecvTimeoutError::Disconnected) => fail(
            &mut child,
            "its output ended",
            Duration::from_secs(5),
            &said,
            stderr,
        ),
        Err(mpsc::RecvTimeoutError::Timeout) => fail(
            &mut child,
            "it stayed silent past the timeout",
            Duration::ZERO,
            &said,
            stderr,
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

/// What the backend writes to stderr, read on a thread of its own
/// until the pipe closes, so the pipe never fills; the receiver hears
/// when it has.
type Drained = (Arc<Mutex<Vec<u8>>>, mpsc::Receiver<()>);

fn drain(mut stderr: std::process::ChildStderr) -> Drained {
    let captured: Arc<Mutex<Vec<u8>>> = Arc::default();
    let sink = Arc::clone(&captured);
    let (done, closed) = mpsc::channel();
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
        let _ = done.send(());
    });
    (captured, closed)
}

/// Fail the start with everything the backend said: its exit status,
/// or that it was still running and has been killed; the stdout it
/// wrote, which carries its log; and its stderr.
///
/// `grace` is how long it may take to exit before it is killed.
fn fail(
    child: &mut Child,
    why: &str,
    grace: Duration,
    said: &Mutex<String>,
    stderr: Option<Drained>,
) -> ! {
    let status = match exited_within(child, grace).or_else(|| child.try_wait().ok().flatten()) {
        Some(status) => format!("exited with {status}"),
        None => {
            let _ = child.kill();
            let _ = child.wait();
            "was still running and has been killed".to_string()
        }
    };
    let stderr = stderr.map_or_else(
        || "<redirected by the caller>".to_string(),
        |(captured, closed)| {
            // The child is gone, so its stderr closes once whatever
            // inherited the pipe is gone too; wait a bounded time for
            // the drain to read the last of it.
            let _ = closed.recv_timeout(Duration::from_secs(2));
            String::from_utf8_lossy(&captured.lock().expect("stderr buffer")).into_owned()
        },
    );
    let stdout = said.lock().expect("stdout so far").clone();
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
