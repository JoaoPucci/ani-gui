//! Tests for the spawn plumbing: the platform teardown command and
//! output cleaning. Moved here with their subject — they never
//! tested the resolver, only how a spawned tool is taken down.

use super::*;

#[cfg(unix)]
#[test]
fn tree_kill_args_addresses_the_process_group() {
    let (prog, args) = tree_kill_args(1234);
    assert_eq!(prog, "kill");
    assert_eq!(args, vec!["-9", "--", "-1234"]);
}

#[cfg(unix)]
proptest::proptest! {
    // For any pid, the command names the negated group.
    #[test]
    fn tree_kill_args_always_names_the_group(pid in proptest::num::u32::ANY) {
        let (_, args) = tree_kill_args(pid);
        let want = format!("-{pid}");
        proptest::prop_assert!(args.iter().any(|a| a == &want), "{:?} missing {}", args, want);
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
    guard.wait().await.expect("the child exits");
    assert!(guard.take_down().await, "the tree is gone");
    let s = state.lock().expect("fake tree");
    assert_eq!(s.kills, 1, "what was left was killed once");
    assert!(!s.running);
}

#[tokio::test]
async fn a_tree_already_gone_is_not_signalled() {
    let (tree, state) = fake_tree(false, None);
    let mut guard = TreeKillChild::with_tree(quick_child(), Box::new(tree));
    guard.wait().await.expect("the child exits");
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
    guard.wait().await.expect("the child exits");
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

#[tokio::test]
async fn a_tree_taken_down_once_is_not_looked_at_again() {
    let (tree, state) = fake_tree(true, Some(0));
    let mut guard = TreeKillChild::with_tree(quick_child(), Box::new(tree));
    guard.wait().await.expect("the child exits");
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
    guard.wait().await.expect("the child exits");
    drop(guard);
    assert_eq!(state.lock().expect("fake tree").kills, 0);
}

#[tokio::test]
async fn a_dropped_guard_whose_tree_never_empties_waits_the_ceiling_once() {
    // The drop's own wait: nothing took the tree down before it, so it
    // kills, waits the ceiling out and gives up rather than hang.
    let (tree, state) = fake_tree(true, None);
    let mut guard = TreeKillChild::with_tree(quick_child(), Box::new(tree));
    guard.wait().await.expect("the child exits");
    let dropped = std::time::Instant::now();
    drop(guard);
    let took = dropped.elapsed();
    assert!(took >= TREE_EXIT_CEILING, "it waited: {took:?}");
    assert!(took < TREE_EXIT_CEILING * 2, "once: {took:?}");
    assert_eq!(state.lock().expect("fake tree").kills, 1);
}

/// A tool whose first act is to start a helper and exit. On Windows a
/// process starts running as soon as it is created, so a tool put in
/// its job only afterwards can start a helper first, and the helper is
/// in no job: once the tool has exited nothing reaches it. A guarded
/// spawn puts the tool in its tree before it runs — on Unix the group
/// is set between fork and exec; on Windows the tool is created
/// suspended and resumed once it is in its jobs.
fn helper_then_exit() -> tokio::process::Command {
    let mut cmd = if cfg!(windows) {
        tokio::process::Command::new("cmd")
    } else {
        tokio::process::Command::new("sh")
    };
    #[cfg(windows)]
    cmd.raw_arg("/C start /B ping -n 30 127.0.0.1 >NUL");
    #[cfg(not(windows))]
    cmd.args(["-c", "sleep 30 >/dev/null 2>&1 & exit 0"]);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    cmd
}

#[tokio::test]
async fn a_helper_the_tool_starts_first_is_in_its_tree() {
    let mut guard = GuardedCommand::new(helper_then_exit())
        .spawn()
        .expect("spawn");
    guard.wait().await.expect("the tool exits");
    assert!(
        guard.tree_running(),
        "the helper it started before anything else is in the tree"
    );
    assert!(guard.take_down().await, "and is taken down with it");
}

/// What the Windows spawn rests on, which no case can time on its own:
/// the job wrapper that creates the tool suspended and resumes it only
/// after every wrapper's post-spawn hook — the outer job's among them —
/// has run; and kill-on-drop, which makes the tool's own job end its
/// members when its handle closes, so a helper the tool leaves is not
/// outside every kill when the outer job could not be made.
#[cfg(windows)]
#[test]
fn a_windows_guarded_command_is_suspended_until_its_jobs_and_ends_them_on_close() {
    let mut cmd = tokio::process::Command::new("cmd");
    cmd.raw_arg("/C exit 0");
    let guarded = GuardedCommand::new(cmd);
    assert!(guarded.wraps::<process_wrap::tokio::JobObject>());
    assert!(guarded.wraps::<process_wrap::tokio::KillOnDrop>());
    assert!(guarded.wraps_outer_job());
}
