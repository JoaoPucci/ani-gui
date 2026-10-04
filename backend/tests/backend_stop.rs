//! A quit stops the tools a running download spawned.
//!
//! The downloader runs yt-dlp or ffmpeg in a process group of its own,
//! so that cancelling a download can take the tool's whole tree down.
//! The same group is why a quit cannot reach it from outside: Electron
//! signals the backend's group, and the tool is not in it. Only the
//! backend can stop its tools, so it has to treat the signal as a
//! request and wind down — which drops the download and runs the guard
//! that kills the tool's group — rather than die on it.
//!
//! The backend here is the real binary, and the download a real one up
//! to the tool: stand-ins beside the binary answer for the provider's
//! transport and for yt-dlp, the way the packages' bundled directory
//! outranks anything installed.
//!
//! Unix only, because the signal is. On Windows a quit runs
//! `taskkill /F /T` against the backend, which walks the tree by
//! parent pid and ends the tools itself; no signal is delivered and
//! the backend has nothing to do.

#![cfg(unix)]

mod common;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// The provider's transport: answers every step of the resolve walk
/// from the script, in the `<body>\n<status> <url>` shape the real
/// one prints. The URL is curl's operand, the last argument.
const TRANSPORT: &str = r##"#!/bin/sh
for url in "$@"; do :; done
case "$url" in
  */browse*) body='<a href="/anime/test-show-1"><img alt="test"/></a>' ;;
  */anime/1/episodes) body='{"episodes":[{"id":1001,"number":1},{"id":1002,"number":2}]}' ;;
  */episode/*/languages) body='{"languages":[{"code":"jpn","embed_url":"https://stand-in.invalid/embed/x"}]}' ;;
  */embed/x) body="player.setup({ file: 'https://stand-in.invalid/op/master.m3u8' });" ;;
  */master.m3u8) body='#EXTM3U' ;;
  *) printf 'not found\n404 %s' "$url"; exit 0 ;;
esac
printf '%s\n200 %s' "$body" "$url"
"##;

/// yt-dlp: a tool with a helper of its own, both of which outlive
/// anything that does not kill the group. Each leaves its pid beside
/// the stand-ins' directory.
const TOOL: &str = r#"#!/bin/sh
here="$(dirname "$0")/.."
sleep 300 &
echo "$!" > "$here/helper.pid.tmp" && mv "$here/helper.pid.tmp" "$here/helper.pid"
echo "$$" > "$here/tool.pid.tmp" && mv "$here/tool.pid.tmp" "$here/tool.pid"
wait
"#;

fn stage(path: &Path, script: &str) {
    std::fs::write(path, script).expect("write stand-in");
    let mut perms = std::fs::metadata(path).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).expect("chmod");
}

/// Whether `pid` is a process still running. Asked of `ps` for its
/// state rather than of `kill -0`, which also answers yes for a
/// process that has died and not been reaped — and where nothing reaps
/// orphans, a container with no init, a killed tool would stay "alive"
/// that way for good.
fn alive(pid: u32) -> bool {
    let Ok(out) = Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return false;
    };
    let state = String::from_utf8_lossy(&out.stdout);
    let state = state.trim();
    !state.is_empty() && !state.starts_with('Z')
}

/// `path` as it can go into a query string: everything but the
/// unreserved characters percent-encoded, so a checkout under a
/// directory with a space or an ampersand in its name still makes one
/// well-formed request.
fn query_value(path: &Path) -> String {
    let mut out = String::new();
    for byte in path.to_string_lossy().bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Kill a tool's whole process group, for cleaning up after a test.
fn kill_group(leader: u32) {
    let _ = Command::new("kill")
        .args(["-9", "--", &format!("-{leader}")])
        .stderr(std::process::Stdio::null())
        .status();
}

/// The pid a stand-in wrote to `file`, once it has.
fn pid_from(file: &Path, wait: Duration) -> Option<u32> {
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        if let Some(pid) = std::fs::read_to_string(file)
            .ok()
            .and_then(|s| s.trim().parse().ok())
        {
            return Some(pid);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

fn gone_within(pids: &[u32], wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        if pids.iter().all(|pid| !alive(*pid)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn signal(name: &str, pid: u32) {
    let sent = Command::new("kill")
        .args([name, &pid.to_string()])
        .status()
        .expect("kill");
    assert!(sent.success(), "kill {name} {pid}");
}

/// A backend with a download running, up to the point where the tool
/// and its helper are both alive.
struct Downloading {
    /// Holds the stand-ins and everything the backend writes; kept
    /// for as long as the download is.
    _dir: tempfile::TempDir,
    backend: common::Backend,
    /// The backend's output, until a test stops reading it.
    output: Option<std::io::BufReader<std::process::ChildStdout>>,
    /// The download's stream. Open for as long as this is held — the
    /// harder case, a client that never hangs up.
    _stream: std::net::TcpStream,
    tool: u32,
    helper: u32,
}

impl Downloading {
    /// Whether the tool and its helper are gone within `wait`; then
    /// leave nothing behind whatever the answer.
    fn tools_gone_within(&mut self, wait: Duration) -> bool {
        let gone = gone_within(&[self.tool, self.helper], wait);
        // Only if they are still there: once gone, the pid may be
        // someone else's.
        if !gone {
            kill_group(self.tool);
        }
        let _ = self.backend.child.kill();
        let _ = self.backend.child.wait();
        gone
    }
}

fn start_a_download() -> Downloading {
    // Under the target directory rather than the system's temporary
    // one: the backend is hard-linked in, which needs one filesystem.
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("tempdir");
    let root = dir.path();
    // The backend looks for its bundled tools in `bin/` beside its own
    // executable, so a private name for the executable gives this test
    // a bundled directory of its own.
    let exe = root.join("ani-gui-backend");
    let built = env!("CARGO_BIN_EXE_ani-gui-backend");
    if std::fs::hard_link(built, &exe).is_err() {
        std::fs::copy(built, &exe).expect("copy backend");
    }
    let bin = root.join("bin");
    std::fs::create_dir(&bin).expect("mkdir bin");
    stage(&bin.join("curl_firefox135"), TRANSPORT);
    stage(&bin.join("yt-dlp"), TOOL);

    // Standard error is a pipe nobody reads, like standard output
    // once a test lets go of it: what a parent that died leaves.
    let (unread, stderr) = std::io::pipe().expect("pipe");
    drop(unread);
    let mut cmd = common::command(&exe, root);
    cmd.env("ANI_GUI_PARENT_STDIN", "1").stderr(stderr);
    let (mut backend, output) = common::start_keeping_output(cmd);

    let authority = backend
        .api_base
        .strip_prefix("http://")
        .expect("an http origin")
        .to_string();
    let mut stream = std::net::TcpStream::connect(&authority).expect("connect");
    let downloads = root.join("downloads");
    write!(
        stream,
        "GET /api/download/stream?title=test&episode=1&mode=sub&quality=best&episode_count=2&download_dir={} HTTP/1.1\r\nHost: {authority}\r\nAccept: text/event-stream\r\n\r\n",
        query_value(&downloads)
    )
    .expect("request");

    let tool = pid_from(&root.join("tool.pid"), Duration::from_secs(60));
    let helper = pid_from(&root.join("helper.pid"), Duration::from_secs(5));
    let (Some(tool), Some(helper)) = (tool, helper) else {
        // Killing the backend skips its guards, so a tool that did
        // start is this test's to clean up.
        if let Some(tool) = tool {
            kill_group(tool);
        }
        let _ = backend.child.kill();
        let _ = backend.child.wait();
        panic!("the download never reached its tool: tool={tool:?} helper={helper:?}");
    };
    assert!(alive(tool) && alive(helper), "the tool and its helper run");
    Downloading {
        _dir: dir,
        backend,
        output: Some(output),
        _stream: stream,
        tool,
        helper,
    }
}

#[test]
fn a_terminate_signal_stops_the_tools_a_running_download_spawned() {
    let mut download = start_a_download();

    // What a quit delivers. To the backend alone: the tool's group is
    // its own, so the signal Electron sends the backend's group never
    // reaches it.
    signal("-TERM", download.backend.child.id());

    let status = common::exited_within(&mut download.backend.child, Duration::from_secs(20));
    let stopped = download.tools_gone_within(Duration::from_secs(5));
    let status = status.expect("the backend exits once it is asked to stop");
    assert!(stopped, "the tool and its helper are gone with the backend");
    assert!(
        status.success(),
        "a requested stop is a clean exit: {status:?}"
    );
}

/// The other way a backend loses its app: the parent dies — a crash, a
/// kill — and never gets to send anything. The parent watch has to
/// reach the same wind-down, with every pipe to the parent gone, the
/// backend's own output among them.
#[test]
fn a_parent_that_dies_stops_the_tools_a_running_download_spawned() {
    let mut download = start_a_download();

    // Everything the parent held, at once: the reader of the backend's
    // output and the pipe the backend watches.
    drop(download.output.take());
    drop(download.backend.child.stdin.take());

    let status = common::exited_within(&mut download.backend.child, Duration::from_secs(20));
    let stopped = download.tools_gone_within(Duration::from_secs(5));
    let status = status.expect("the backend exits once its parent is gone");
    assert!(stopped, "the tool and its helper are gone with the backend");
    assert!(
        status.success(),
        "a parent gone is a clean exit: {status:?}"
    );
}

/// The signals a backend run by hand meets — Ctrl+C, and its terminal
/// closing — end it the same way, for the same reason: a tool it
/// started is in a group of its own and neither signal reaches it.
#[test]
fn an_interrupt_or_a_hangup_is_a_request_to_stop_as_well() {
    let home = tempfile::tempdir().expect("tempdir");
    for name in ["-INT", "-HUP"] {
        let mut backend = common::start(common::command(
            Path::new(env!("CARGO_BIN_EXE_ani-gui-backend")),
            home.path(),
        ));
        signal(name, backend.child.id());
        let status = common::exited_within(&mut backend.child, Duration::from_secs(10));
        let _ = backend.child.kill();
        let _ = backend.child.wait();
        let status = status.expect("the backend exits once it is asked to stop");
        assert!(
            status.success(),
            "{name} is a request the backend honours, not what kills it: {status:?}"
        );
    }
}
