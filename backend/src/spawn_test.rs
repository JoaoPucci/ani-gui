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

/// A tree whose running members the case decides, standing in for
/// the platform's: running until killed and gone a few looks after
/// that, or never gone. What the guard does with it is the same on
/// every platform; only how a tree is looked at and killed is not.
struct FakeTree {
    state: std::sync::Arc<std::sync::Mutex<FakeState>>,
}

#[derive(Default)]
struct FakeState {
    running: bool,
    kills: u32,
    looks_since_kill: u32,
    gone_after_looks: Option<u32>,
}

impl Tree for FakeTree {
    fn running(&mut self) -> bool {
        let mut s = self.state.lock().expect("fake tree");
        if s.kills > 0 {
            if let Some(n) = s.gone_after_looks {
                if s.looks_since_kill >= n {
                    s.running = false;
                }
            }
            s.looks_since_kill += 1;
        }
        s.running
    }

    fn kill(&mut self) {
        self.state.lock().expect("fake tree").kills += 1;
    }
}

fn fake_tree(
    running: bool,
    gone_after_looks: Option<u32>,
) -> (FakeTree, std::sync::Arc<std::sync::Mutex<FakeState>>) {
    let state = std::sync::Arc::new(std::sync::Mutex::new(FakeState {
        running,
        gone_after_looks,
        ..FakeState::default()
    }));
    (
        FakeTree {
            state: state.clone(),
        },
        state,
    )
}

/// A child that exits at once by itself, on either platform.
fn quick_child() -> tokio::process::Child {
    let mut cmd = if cfg!(windows) {
        let mut c = tokio::process::Command::new("cmd");
        c.args(["/C", "exit 0"]);
        c
    } else {
        tokio::process::Command::new("true")
    };
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn")
}

#[tokio::test]
async fn a_tree_left_running_by_a_child_that_exited_is_taken_down_and_waited_for() {
    // The tool exited by itself and left a helper running in its
    // tree. The teardown kills what is left and returns once it is
    // gone, whichever platform's tree it is.
    let (tree, state) = fake_tree(true, Some(3));
    let mut guard = TreeKillChild::with_tree(quick_child(), Box::new(tree));
    guard.child_mut().wait().await.expect("the child exits");
    assert!(guard.take_down().await, "the tree is gone");
    let s = state.lock().expect("fake tree");
    assert_eq!(s.kills, 1, "what was left was killed once");
    assert!(!s.running);
}

#[tokio::test]
async fn a_tree_already_gone_is_not_signalled() {
    let (tree, state) = fake_tree(false, None);
    let mut guard = TreeKillChild::with_tree(quick_child(), Box::new(tree));
    guard.child_mut().wait().await.expect("the child exits");
    assert!(guard.take_down().await);
    assert_eq!(state.lock().expect("fake tree").kills, 0);
    drop(guard);
    assert_eq!(
        state.lock().expect("fake tree").kills,
        0,
        "nor by the drop once the tree is known gone"
    );
}

#[tokio::test]
async fn a_tree_that_never_empties_is_given_up_and_the_drop_does_not_wait_again() {
    let (tree, state) = fake_tree(true, None);
    let mut guard = TreeKillChild::with_tree(quick_child(), Box::new(tree));
    guard.child_mut().wait().await.expect("the child exits");
    assert!(!guard.take_down().await, "past the ceiling it is given up");
    let dropped = std::time::Instant::now();
    drop(guard);
    assert!(
        dropped.elapsed() < TREE_EXIT_CEILING / 2,
        "the drop waited the ceiling a second time"
    );
    assert_eq!(
        state.lock().expect("fake tree").kills,
        2,
        "the drop asks for the kill again"
    );
}

/// The real Windows tree: a tool that exits by itself leaving a
/// helper running. Before the job, the helper was out of reach once
/// its root had exited — `taskkill /T` finds a tree through a live
/// root — so it went on running beside whatever followed.
#[cfg(windows)]
#[tokio::test]
async fn a_windows_tool_that_exits_leaving_a_helper_has_the_helper_taken_down() {
    // Raw, so cmd reads its own command line rather than one quoted
    // for a C runtime's parser.
    let child = tokio::process::Command::new("cmd")
        .raw_arg("/C start /B ping -n 30 127.0.0.1 >NUL")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn");
    let mut guard = TreeKillChild::new(child);
    guard.child_mut().wait().await.expect("cmd exits");
    assert!(
        guard.tree_running(),
        "the helper is still running in the tree"
    );
    // The teardown returns true only once the job lists no member.
    assert!(guard.take_down().await, "the helper is taken down");
}

#[tokio::test]
async fn a_tree_taken_down_once_is_not_looked_at_again() {
    let (tree, state) = fake_tree(true, Some(0));
    let mut guard = TreeKillChild::with_tree(quick_child(), Box::new(tree));
    guard.child_mut().wait().await.expect("the child exits");
    assert!(guard.take_down().await);
    assert!(
        guard.take_down().await,
        "a second teardown has nothing to do"
    );
    assert_eq!(state.lock().expect("fake tree").kills, 1);
}

#[tokio::test]
async fn a_dropped_guard_whose_tree_is_already_gone_sends_nothing() {
    let (tree, state) = fake_tree(false, None);
    let mut guard = TreeKillChild::with_tree(quick_child(), Box::new(tree));
    guard.child_mut().wait().await.expect("the child exits");
    drop(guard);
    assert_eq!(state.lock().expect("fake tree").kills, 0);
}

#[tokio::test]
async fn a_dropped_guard_whose_tree_never_empties_waits_the_ceiling_once() {
    // The drop's own wait: nothing took the tree down before it, so it
    // kills, waits the ceiling out and gives up rather than hang.
    let (tree, state) = fake_tree(true, None);
    let mut guard = TreeKillChild::with_tree(quick_child(), Box::new(tree));
    guard.child_mut().wait().await.expect("the child exits");
    let dropped = std::time::Instant::now();
    drop(guard);
    let took = dropped.elapsed();
    assert!(took >= TREE_EXIT_CEILING, "it waited: {took:?}");
    assert!(took < TREE_EXIT_CEILING * 2, "once: {took:?}");
    assert_eq!(state.lock().expect("fake tree").kills, 1);
}

#[test]
fn a_windows_root_gets_its_tree_kill_only_while_it_is_in_its_job() {
    // `taskkill /T` reaches what the root started before it joined its
    // job, but only through a live root. A root that has exited is
    // gone from the job's members, and its pid may already belong to
    // another program, whose tree the kill would take down.
    assert!(tree::kills_root_tree(Some(&[9, 7]), 9), "still running");
    assert!(
        !tree::kills_root_tree(Some(&[7]), 9),
        "exited: only the members it left"
    );
    assert!(!tree::kills_root_tree(Some(&[]), 9));
}

#[test]
fn a_windows_root_without_a_job_gets_its_tree_kill() {
    // No job, so no members to go by: the guard only asks for the
    // kill while the child is unreaped, and its pid is still its own.
    assert!(tree::kills_root_tree(None, 9));
}

#[test]
fn a_windows_kill_closes_the_inner_job_while_it_is_open() {
    // Closing the inner job has the kernel end every process in it at
    // once — no pid is looked up, so none can have been reused. The
    // outer job stays open to say when they are gone.
    let plan = tree::kill_plan(Some(&[9, 7]), 9, true);
    assert!(plan.close_inner);
    assert!(plan.each_member.is_empty(), "no pid lookups: {plan:?}");
    assert!(plan.tree_kill_root, "the root is still a member");
}

#[test]
fn a_windows_kill_without_an_inner_job_falls_back_to_each_member() {
    // Already closed — a second kill after the ceiling — or never
    // made: what the outer job still lists is killed one by one.
    let plan = tree::kill_plan(Some(&[7, 5]), 9, false);
    assert!(!plan.close_inner);
    assert_eq!(plan.each_member, vec![7, 5]);
    assert!(!plan.tree_kill_root, "the root has left the job");
}

#[test]
fn a_windows_kill_without_any_job_reaches_the_tree_through_the_root() {
    let plan = tree::kill_plan(None, 9, false);
    assert!(plan.tree_kill_root);
    assert!(!plan.close_inner);
    assert!(plan.each_member.is_empty());
}
