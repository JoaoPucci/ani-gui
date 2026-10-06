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
    // download must take yt-dlp and the ffmpeg it spawns for merges
    // down with it.
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

#[test]
fn taskkill_report_names_the_tree_and_not_the_backend() {
    // The root's line names the backend as its parent; the backend
    // is not part of the tree it is waiting on. Nor is the root: its
    // own handle says when it has exited, and a lookup by pid could
    // still find it while that handle is held.
    let report = "SUCCESS: The process with PID 4120 (child process of PID 3008) has been terminated.\r\n\
                  SUCCESS: The process with PID 3008 (child process of PID 77) has been terminated.\r\n";
    assert_eq!(pids_taken_down(report, 77, 3008), vec![4120]);
}

#[test]
fn taskkill_report_never_names_the_systems_own_processes() {
    // Pids 0 and 4 are the idle process and System, always running;
    // a report misread into one of them would hold every teardown to
    // its ceiling.
    let report = "SUCCESS: PID 0 PID 4 PID 4120 (child process of PID 3008)\r\n";
    assert_eq!(pids_taken_down(report, 77, 3008), vec![4120]);
}

#[test]
fn taskkill_report_in_another_language_names_the_same_tree() {
    let report = "ERFOLGREICH: Der Prozess mit PID 4120 (untergeordneter Prozess von PID 3008) wurde beendet.\r\n\
                  ERFOLGREICH: Der Prozess mit PID 3008 (untergeordneter Prozess von PID 77) wurde beendet.\r\n";
    assert_eq!(pids_taken_down(report, 77, 3008), vec![4120]);
}

#[test]
fn tasklist_query_asks_for_one_pid_as_csv_without_a_header() {
    assert_eq!(
        tasklist_args(4120),
        ["/FI", "PID eq 4120", "/FO", "CSV", "/NH"].map(String::from)
    );
}

#[test]
fn tasklist_row_for_the_pid_is_a_running_process() {
    let row = "\"yt-dlp.exe\",\"4120\",\"Console\",\"1\",\"12,345 K\"\r\n";
    assert!(tasklist_shows(row, 4120));
    assert!(!tasklist_shows(row, 412));
    assert!(!tasklist_shows(row, 1));
    let comma = "\"a,1.exe\",\"4120\",\"Console\",\"1\",\"12,345 K\"\r\n";
    assert!(tasklist_shows(comma, 4120));
    assert!(!tasklist_shows(comma, 1));
}

#[test]
fn tasklist_answer_naming_no_process_is_not_a_running_one() {
    let none = "INFO: No tasks are running which match the specified criteria.\r\n";
    assert!(!tasklist_shows(none, 4120));
}

proptest::proptest! {
    // Whatever the report's wording, every pid in it is waited on but
    // the backend's, the root's and the system's own, each once.
    #[test]
    fn taskkill_report_yields_each_pid_once_but_the_backend(
        pids in proptest::collection::vec(0u32..100_000, 0..8),
        own in 1u32..100_000,
        root in 1u32..100_000,
        words in "[A-Za-zÄÖÜäöü :().-]{0,20}",
    ) {
        let report: String = pids
            .iter()
            .map(|p| format!("{words} PID {p} {words}\r\n"))
            .collect();
        let mut want: Vec<u32> = pids
            .iter()
            .copied()
            .filter(|&p| p != own && p != root && p != 0 && p != 4)
            .collect();
        want.sort_unstable();
        want.dedup();
        proptest::prop_assert_eq!(pids_taken_down(&report, own, root), want);
    }

    // A row is a running process exactly when its pid field is the
    // pid asked about, whatever the image name around it.
    #[test]
    fn tasklist_row_shows_exactly_its_own_pid(
        image in "[A-Za-z0-9_., -]{1,20}",
        row_pid in 1u32..100_000,
        asked in 1u32..100_000,
    ) {
        let row = format!("\"{image}\",\"{row_pid}\",\"Console\",\"1\",\"1 K\"\r\n");
        proptest::prop_assert_eq!(tasklist_shows(&row, asked), row_pid == asked);
    }
}
