//! Tests for the spawn plumbing: the platform teardown command and
//! output cleaning. Moved here with their subject — they never
//! tested the resolver, only how a spawned tool is taken down.

use super::*;

#[test]
fn tree_kill_args_unix_addresses_the_process_group() {
    let (prog, args) = tree_kill_args(1234, false).expect("unix tree kill");
    assert_eq!(prog, "kill");
    assert_eq!(args, vec!["-9", "--", "-1234"]);
}

#[test]
fn tree_kill_args_windows_kills_the_tree_by_parent_pid() {
    // Windows is a shipped target (package:win) and kill_on_drop
    // only terminates the Git Bash parent there — cancelling a
    // download must take aria2c / ffmpeg / yt-dlp down with it.
    // taskkill /T walks the child tree by parent pid; /F because
    // the transfer tools ignore the graceful signal mid-write.
    let (prog, args) = tree_kill_args(1234, true).expect("windows tree kill");
    assert_eq!(prog, "taskkill");
    assert_eq!(args, vec!["/PID", "1234", "/T", "/F"]);
}

proptest::proptest! {
    // Contract for any pid on both platforms: the command always
    // names the pid (negated group on unix, bare tree root on
    // windows) and never comes back empty — every supported
    // platform has a tree kill.
    #[test]
    fn tree_kill_args_always_names_the_pid(
        pid in proptest::num::u32::ANY,
        windows in proptest::bool::ANY,
    ) {
        let (prog, args) = tree_kill_args(pid, windows).expect("tree kill exists");
        let want = if windows { pid.to_string() } else { format!("-{pid}") };
        let named = args.iter().any(|a| a == &want);
        proptest::prop_assert!(named, "{} args {:?} missing {}", prog, args, want);
    }
}
