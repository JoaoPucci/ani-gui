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

fn alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
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

#[test]
fn a_terminate_signal_stops_the_tools_a_running_download_spawned() {
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

    let mut cmd = common::command(&exe, root);
    cmd.env("ANI_GUI_PARENT_STDIN", "1");
    let mut backend = common::start(cmd);

    // Start a download and keep its stream open for the whole test —
    // the harder case, a client that never hangs up.
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
        downloads.display()
    )
    .expect("request");

    let tool = pid_from(&root.join("tool.pid"), Duration::from_secs(60));
    let helper = pid_from(&root.join("helper.pid"), Duration::from_secs(5));
    let (Some(tool), Some(helper)) = (tool, helper) else {
        let _ = backend.child.kill();
        panic!("the download never reached its tool: tool={tool:?} helper={helper:?}");
    };
    assert!(alive(tool) && alive(helper), "the tool and its helper run");

    // What a quit delivers. To the backend alone: the tool's group is
    // its own, so the signal Electron sends the backend's group never
    // reaches it.
    let term = Command::new("kill")
        .args(["-TERM", &backend.child.id().to_string()])
        .status()
        .expect("kill");
    assert!(term.success());

    let status = common::exited_within(&mut backend.child, Duration::from_secs(20));
    let stopped = gone_within(&[tool, helper], Duration::from_secs(5));

    // Leave nothing behind whatever the outcome.
    let _ = Command::new("kill")
        .args(["-9", "--", &format!("-{tool}")])
        .stderr(std::process::Stdio::null())
        .status();
    let _ = backend.child.kill();
    let _ = backend.child.wait();
    drop(stream);

    let status = status.expect("the backend exits once it is asked to stop");
    assert!(stopped, "the tool and its helper are gone with the backend");
    assert!(
        status.success(),
        "a requested stop is a clean exit: {status:?}"
    );
}
