//! Running an external tool and reading what it prints.
//!
//! Process-group lifecycle and output cleaning for the binaries this
//! backend spawns: the downloader's yt-dlp and ffmpeg, the external
//! player, and Syncplay. It lived under `anicli/` only because the
//! first subprocess driver was written there, and the native paths
//! reached across the module boundary to reuse it.

/// Owns a spawned child and kills its whole process tree when dropped
/// mid-run. Ownership is the point: a struct's `Drop` body runs BEFORE
/// its fields drop, so the tree walk / group signal fires while the
/// child is still alive — on Windows `taskkill /T` can only discover
/// descendants by a live parent pid, and `kill_on_drop`'s SIGKILL (the
/// `Child` field's own drop) must come second under every cancellation
/// mode: task abort, timeout, panic. The signal goes through `kill(1)`
/// / `taskkill(1)` rather than a syscall — the crate forbids unsafe
/// code, and both binaries ship with any host this app runs on. Each
/// is run to completion, so neither lingers as a zombie; they exit in
/// microseconds.
///
/// Either command only asks: it returns once the kill is requested,
/// and the processes exit when the kernel gets to them. Whatever
/// follows a teardown — a respawn on the same resume state, a retry
/// beside it, the sweep of what the tool wrote — needs the exit, so
/// a teardown waits for it ([`Self::take_down`], and the drop, which
/// waits the same way blocking), under [`TREE_EXIT_CEILING`].
///
/// A child that exits by itself is not its whole tree. On Unix the
/// guard keeps the group's id past the child's reap: a group with a
/// process still in it keeps its id from being handed out again, so
/// signalling the group stays safe until it is empty, and a helper
/// left running in it is taken down too. On Windows the tree is found
/// through the live root, so once the root has exited what it left
/// running is out of the guard's reach.
pub(crate) struct TreeKillChild {
    pub(crate) child: tokio::process::Child,
    /// The child's pid, which on Unix is its group's id; `None` once
    /// the tree is known to be gone.
    pid: Option<u32>,
    /// A teardown already waited the ceiling out: the drop asks for
    /// the kill again but does not wait a second time.
    given_up: bool,
}

impl TreeKillChild {
    pub(crate) fn new(child: tokio::process::Child) -> Self {
        let pid = child.id();
        Self {
            child,
            pid,
            given_up: false,
        }
    }

    /// The guarded child, for the caller's own I/O and wait.
    pub(crate) fn child_mut(&mut self) -> &mut tokio::process::Child {
        &mut self.child
    }

    /// Take the tree down and wait until it has exited: the child
    /// reaped and, of the rest of its tree, nothing left — which is
    /// also what it checks first, so a tree already gone is not
    /// signalled. `false` when the ceiling passed first: the kill was
    /// asked for, but the tree cannot be said to be gone, and nothing
    /// that would meet it should start.
    pub(crate) async fn take_down(&mut self) -> bool {
        let Some(pid) = self.pid else {
            return true;
        };
        if self.tree_exited(pid, &[]) {
            self.pid = None;
            return true;
        }
        let tree = kill_process_tree(pid);
        let deadline = std::time::Instant::now() + TREE_EXIT_CEILING;
        loop {
            if self.tree_exited(pid, &tree) {
                self.pid = None;
                return true;
            }
            if std::time::Instant::now() >= deadline {
                tracing::warn!(pid, "a tool's process tree outlived its teardown");
                self.given_up = true;
                return false;
            }
            tokio::time::sleep(TREE_EXIT_POLL).await;
        }
    }

    /// [`Self::take_down`] for the drop, which cannot await: the same
    /// wait, sleeping the thread. A teardown's exit takes milliseconds.
    fn take_down_blocking(&mut self) {
        let Some(pid) = self.pid else { return };
        if self.given_up {
            kill_process_tree(pid);
            return;
        }
        if self.tree_exited(pid, &[]) {
            return;
        }
        let tree = kill_process_tree(pid);
        let deadline = std::time::Instant::now() + TREE_EXIT_CEILING;
        while !self.tree_exited(pid, &tree) {
            if std::time::Instant::now() >= deadline {
                tracing::warn!(pid, "a tool's process tree outlived its teardown");
                return;
            }
            std::thread::sleep(TREE_EXIT_POLL);
        }
    }

    /// The child first, reaping it if it has exited, then the rest of
    /// its tree: on Unix the process group it leads, on Windows the
    /// processes `taskkill` named as it took the tree down.
    fn tree_exited(&mut self, pid: u32, tree: &[u32]) -> bool {
        if matches!(self.child.try_wait(), Ok(None)) {
            return false;
        }
        !tree_alive(pid, tree)
    }
}

impl Drop for TreeKillChild {
    fn drop(&mut self) {
        // A tree already known to be gone reads `None` and is left
        // alone; anything else is taken down, or checked to be gone.
        self.take_down_blocking();
    }
}

/// How long a teardown waits for the tree to exit. A killed process
/// exits in milliseconds; this bounds one stuck in the kernel. One
/// guard's wait stays under the backend's teardown limit, which a
/// guard dropped at shutdown runs inside; several stuck trees at once
/// would not, and the limit then cuts their waits short — the kills
/// were already asked for.
pub(crate) const TREE_EXIT_CEILING: std::time::Duration = std::time::Duration::from_secs(2);

const TREE_EXIT_POLL: std::time::Duration = std::time::Duration::from_millis(10);

/// Whether anything of a taken-down tree is still running, the child
/// itself aside. Unix: the process group the child led — its helpers
/// are in it, and a group no process is in no longer exists. Windows:
/// the processes the teardown named, each looked up by pid.
fn tree_alive(pid: u32, tree: &[u32]) -> bool {
    if cfg!(windows) {
        return tree.iter().any(|&p| {
            std::process::Command::new("tasklist")
                .args(tasklist_args(p))
                .output()
                .is_ok_and(|o| tasklist_shows(&String::from_utf8_lossy(&o.stdout), p))
        });
    }
    std::process::Command::new("kill")
        .args(["-0", "--", &format!("-{pid}")])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// `tasklist`'s query for one pid, as CSV without a header, so a
/// running process is a row whose second field is its pid whatever
/// the system's language.
fn tasklist_args(pid: u32) -> [String; 5] {
    [
        "/FI".into(),
        format!("PID eq {pid}"),
        "/FO".into(),
        "CSV".into(),
        "/NH".into(),
    ]
}

/// Whether `tasklist`'s CSV answer lists `pid`. Every field is
/// quoted, and a field may hold a comma — an image name can — but no
/// quote, so fields part at `","`. An answer naming no process is a
/// sentence in the system's language, with no quoted field in it.
fn tasklist_shows(stdout: &str, pid: u32) -> bool {
    let field = pid.to_string();
    stdout
        .lines()
        .any(|row| row.starts_with('"') && row.split("\",\"").nth(1) == Some(field.as_str()))
}

/// The processes `taskkill /T` reports taking down, to be looked up
/// until they are gone: every pid its report names, whatever the
/// language it is written in — a line per process, naming it and the
/// parent it was found under. Left out: the backend, which the report
/// names as the root's parent and is not part of the tree; the root,
/// whose own handle says when it has exited; and 0 and 4, the idle
/// process and System, which never exit.
fn pids_taken_down(stdout: &str, own: u32, root: u32) -> Vec<u32> {
    let mut pids: Vec<u32> = stdout
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|n| n.parse().ok())
        .filter(|&p| p != own && p != root && p != 0 && p != 4)
        .collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

/// Take down a spawned downloader's whole process tree.
///
/// Shared by the drop guard and by the in-flight stop, so both paths
/// use the same platform command and the same test seam. Returns the
/// processes the teardown named, which only Windows reports and only
/// Windows needs: a tree there has no group to look for afterwards.
pub(crate) fn kill_process_tree(pid: u32) -> Vec<u32> {
    #[cfg(test)]
    if let Some(probe) = tree_kill_probe() {
        let _ = std::process::Command::new(probe)
            .arg(pid.to_string())
            .status();
        return Vec::new();
    }
    let Some((prog, args)) = tree_kill_args(pid, cfg!(windows)) else {
        return Vec::new();
    };
    let Ok(out) = std::process::Command::new(prog).args(&args).output() else {
        return Vec::new();
    };
    if cfg!(windows) {
        pids_taken_down(
            &String::from_utf8_lossy(&out.stdout),
            std::process::id(),
            pid,
        )
    } else {
        Vec::new()
    }
}

/// Test seam: when a probe is registered, the teardown runs it (child
/// pid as its argument) INSTEAD of the real tree-kill command. The
/// Windows contract — `taskkill /T` can only discover descendants
/// while the parent is still alive — is unobservable on the platforms
/// the suite runs on, so a probe standing in for the kill command is
/// the only way a test can record WHEN the teardown fires relative to
/// the parent's reap. The probe takes over the cleanup duty too.
#[cfg(test)]
pub(crate) static TREE_KILL_PROBE: std::sync::Mutex<Option<std::path::PathBuf>> =
    std::sync::Mutex::new(None);

/// Serializes the probe's scope: a test that REGISTERS a probe (and
/// so redirects every teardown in the process) and a test that needs
/// the REAL teardown to run must not overlap — a no-op probe held by
/// one would silently swallow the other's kill. Held for the whole
/// test either way.
#[cfg(all(test, unix))]
pub(crate) static TREE_KILL_PROBE_SCOPE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(test)]
fn tree_kill_probe() -> Option<std::path::PathBuf> {
    TREE_KILL_PROBE.lock().expect("probe lock").clone()
}

/// Platform command that takes down a spawned downloader's whole
/// process tree. Unix: `kill -9 -- -PID` — the negative pid addresses
/// the process group created at spawn (`process_group(0)`, pgid ==
/// child pid). Windows: `taskkill /PID <pid> /T /F` — no process
/// group there; /T walks the child tree by parent pid (which is why
/// the guard must fire while the shell is still alive), /F because
/// the transfer tools ignore the graceful signal mid-write.
fn tree_kill_args(pid: u32, windows: bool) -> Option<(&'static str, Vec<String>)> {
    if windows {
        return Some((
            "taskkill",
            vec!["/PID".into(), pid.to_string(), "/T".into(), "/F".into()],
        ));
    }
    Some(("kill", vec!["-9".into(), "--".into(), format!("-{pid}")]))
}

/// Strip ANSI escape sequences from a byte slice and decode lossy UTF-8.
#[must_use]
pub fn strip_ansi(bytes: &[u8]) -> String {
    let cleaned = strip_ansi_escapes::strip(bytes);
    String::from_utf8_lossy(&cleaned).into_owned()
}

#[cfg(test)]
#[path = "spawn_test.rs"]
mod tests;
