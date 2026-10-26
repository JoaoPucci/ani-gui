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
    /// Every look at the tree, before a kill or after.
    looks: u32,
    looks_since_kill: u32,
    gone_after_looks: Option<u32>,
}

impl Tree for FakeTree {
    fn running(&mut self) -> bool {
        let mut s = self.state.lock().expect("fake tree");
        s.looks += 1;
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
    // gone, whichever platform's tree it is. What is asserted is that
    // it waited for the tree, not how fast: under the app's ceiling a
    // runner that stalls the case past it fails it with nothing wrong.
    let (tree, state) = fake_tree(true, Some(3));
    let mut guard =
        TreeKillChild::with_tree(quick_child(), Box::new(tree)).with_ceiling(CASE_DEADLINE);
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
    // Waiting is looking: the guard's wait looks at the tree until it
    // is gone or the ceiling passes. A drop that does not look has not
    // waited, however long the machine took to run it.
    let looked = state.lock().expect("fake tree").looks;
    drop(guard);
    let s = state.lock().expect("fake tree");
    assert_eq!(
        s.looks, looked,
        "the drop waited for the tree a second time"
    );
    assert_eq!(s.kills, 2, "the drop asks for the kill again");
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
    // A wait ends at the ceiling or after it, never before, however
    // fast the machine; how far after is the scheduler's business. Each
    // of the guard's waits starts with a kill, so one kill is one wait.
    assert!(took >= TREE_EXIT_CEILING, "it waited: {took:?}");
    let s = state.lock().expect("fake tree");
    assert_eq!(s.kills, 1, "it waited once");
    assert!(s.looks > 1, "it looked at the tree while it waited");
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
    // A real kill and reap: under a failure bound of the case's own,
    // since the app's ceiling is one a loaded runner can stall past,
    // and whatever probe another case has registered may serve this
    // teardown and land its kill late.
    let mut guard = GuardedCommand::new(helper_then_exit())
        .spawn()
        .expect("spawn")
        .with_ceiling(CASE_DEADLINE);
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

/// How long a case waits for a step the subject takes on its own before
/// it fails: long enough for any machine to get there, so only a step
/// that never comes ends the case.
const CASE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

/// Releases a held probe run when dropped, and withdraws the probe, so
/// a case that fails before it lets go leaves no run waiting and no
/// probe registered whose script is about to be removed — every later
/// teardown in the binary would run that missing script and send no
/// kill.
struct Released(std::path::PathBuf);

impl Drop for Released {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, b"");
        *TREE_KILL_PROBE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}

/// A case that registers a probe withdraws it as it ends and removes
/// the script with its directory. A teardown elsewhere in the run may
/// be running that probe in that moment — read before the withdrawal,
/// executed after it — and a script gone between the two is a kill
/// that never happens: the tree it stood in for, another case's,
/// outlives its teardown. The withdrawal waits for a probe run in
/// flight to end.
#[cfg(unix)]
#[test]
fn a_probe_is_not_withdrawn_while_a_teardown_is_running_it() {
    use std::os::unix::fs::PermissionsExt;
    let _scope = TREE_KILL_PROBE_SCOPE.blocking_lock();
    let dir = tempfile::tempdir().expect("probe dir");
    let started = dir.path().join("started");
    let release = dir.path().join("release");
    let probe = dir.path().join("probe");
    // Every case's teardown in the binary is served by whatever probe
    // is registered, so this one kills any tree but the placeholder's
    // at once, as a real teardown would. The placeholder's run is held
    // until the case releases it, and only then ends: a run that ended
    // by itself first — after so many looks, on a machine slow enough —
    // would let the withdrawal through early and fail the case for
    // nothing. The case releases it however it ends ([`Released`]); the
    // bound on the looks is there only so a test binary killed outright
    // leaves no probe behind, minutes past any case's own deadline.
    let _released = Released(release.clone());
    std::fs::write(
        &probe,
        format!(
            "#!/bin/sh\nif [ \"$1\" != 4000000 ]; then kill -s KILL -- -\"$1\" 2>/dev/null; exit 0; fi\n: > {started}\ni=0\nwhile [ ! -f {release} ] && [ \"$i\" -lt 30000 ]; do sleep 0.01; i=$((i+1)); done\nexit 0\n",
            started = crate::stub_file::sh_quote(&started),
            release = crate::stub_file::sh_quote(&release),
        ),
    )
    .expect("write probe");
    std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    *TREE_KILL_PROBE.lock().expect("probe lock") = Some(probe);
    // The pid is only the probe's argument: this probe signals nothing.
    let teardown = std::thread::spawn(|| kill_process_tree(4_000_000));
    let deadline = std::time::Instant::now() + CASE_DEADLINE;
    while !started.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "the teardown never ran the probe"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    // The withdrawal says whether it found the probe held before it
    // waits for it: a withdrawal still on its way has not withdrawn
    // anything either, and proves nothing. Held is not held by this
    // teardown alone: every case's teardown takes the probe's lock for
    // a moment, so a teardown that let go while running the probe could
    // still be met holding by another case's. The check sees the
    // withdrawal wait; it cannot always say for whom.
    let (found, on_found) = std::sync::mpsc::channel();
    let (withdrawn, on_withdrawn) = std::sync::mpsc::channel();
    let withdraw = std::thread::spawn(move || {
        let mut probe = match TREE_KILL_PROBE.try_lock() {
            Ok(probe) => {
                let _ = found.send(false);
                probe
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                let _ = found.send(true);
                TREE_KILL_PROBE.lock().expect("probe lock")
            }
            Err(std::sync::TryLockError::Poisoned(_)) => panic!("probe lock poisoned"),
        };
        *probe = None;
        let _ = withdrawn.send(());
    });
    let held = on_found
        .recv_timeout(CASE_DEADLINE)
        .expect("the withdrawal asked for the probe");
    let early = on_withdrawn.try_recv();
    std::fs::write(&release, b"").expect("release");
    let late = on_withdrawn.recv_timeout(CASE_DEADLINE);
    teardown.join().expect("teardown");
    withdraw.join().expect("withdraw");
    assert!(
        held,
        "the withdrawal found the probe free while a teardown was running it"
    );
    assert!(
        early.is_err(),
        "the probe was withdrawn while a teardown was running it"
    );
    assert!(late.is_ok(), "and is withdrawn once the run has ended");
}

/// Linux only: ETXTBSY on exec of a write-open file is a Linux
/// guarantee. macOS permits the exec, so there is no race to wait out
/// and the assertion below would describe nothing.
///
/// A case stages its probe a moment before a teardown runs it, and a
/// fork anywhere else in the test binary in that moment carries the
/// script's write descriptor into the child until the child execs.
/// A teardown that execs the probe then is told the file is busy, and
/// a teardown that takes that for done sends no kill at all: the tree
/// the probe stands in for outlives its teardown, and the case that
/// waits on the probe's run never sees it. The teardown waits a busy
/// probe out, as every spawn of a freshly staged tool does.
#[cfg(target_os = "linux")]
#[test]
fn a_busy_probe_is_waited_out_rather_than_skipped() {
    use std::os::unix::fs::PermissionsExt;
    let _scope = TREE_KILL_PROBE_SCOPE.blocking_lock();
    let dir = tempfile::tempdir().expect("probe dir");
    let ran = dir.path().join("ran");
    let probe = dir.path().join("probe");
    // Any other case's tree is killed at once, as a real teardown
    // would; the placeholder's run is only recorded.
    std::fs::write(
        &probe,
        format!(
            "#!/bin/sh\nif [ \"$1\" != 4000001 ]; then kill -s KILL -- -\"$1\" 2>/dev/null; exit 0; fi\n: > {ran}\nexit 0\n",
            ran = crate::stub_file::sh_quote(&ran),
        ),
    )
    .expect("write probe");
    std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let held = std::fs::OpenOptions::new()
        .write(true)
        .open(&probe)
        .expect("hold the probe open for writing");
    // Self-evidencing: the race is live on this host, or the case
    // below proves nothing.
    let plain = std::process::Command::new(&probe).status();
    assert!(
        matches!(
            plain.as_ref().err().map(std::io::Error::kind),
            Some(std::io::ErrorKind::ExecutableFileBusy)
        ),
        "a write-open probe must be busy for this case to mean anything: {plain:?}"
    );
    *TREE_KILL_PROBE.lock().expect("probe lock") = Some(probe.clone());
    // Let go once the teardown has found the probe busy: one that came
    // to it after the release would run it at once and retry nothing.
    let queue = crate::held::queue(probe.display().to_string());
    let releaser = std::thread::spawn(move || {
        queue.joined_blocking(|| false);
        drop(held);
    });
    // The pid is only the probe's argument: this probe signals nothing.
    kill_process_tree(4_000_001);
    let released = releaser.join();
    *TREE_KILL_PROBE.lock().expect("probe lock") = None;
    assert!(
        released.is_ok(),
        "the teardown never found the probe busy, so it never waited"
    );
    assert!(
        ran.exists(),
        "a busy probe must be waited out, not taken for a kill sent"
    );
}
