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
/// left running in it is taken down too. The group is signalled only
/// after a look finds someone in it; an empty group answers that look
/// with no such process, and nothing is sent. What the look cannot
/// rule out is the id being freed and handed to a new group leader in
/// the moment between it and the kill — a full turn of the pid space
/// in milliseconds. On Windows the tool is put in a job object as it
/// starts, and the processes it starts after that join the job: the
/// tree is the job's members, there whether or not the tool itself is
/// still running. The job is made to kill its members when its last
/// handle closes, which is the guard's, so a backend that ends
/// without running any guard — a crash — still takes its tools with
/// it.
pub(crate) struct TreeKillChild {
    pub(crate) child: tokio::process::Child,
    /// The rest of the child's tree; `None` once it is known to be
    /// gone.
    tree: Option<Box<dyn Tree>>,
    /// A teardown already waited the ceiling out: the drop asks for
    /// the kill again but does not wait a second time.
    given_up: bool,
}

/// What a guard can do with a child's tree beyond the child itself:
/// look for anything of it still running, and ask all of it to die.
/// The guard's logic is the same on every platform; this is what
/// differs.
pub(crate) trait Tree: Send {
    /// Whether any process of the tree is still running.
    fn running(&mut self) -> bool;
    /// Ask every process of the tree still running to die.
    fn kill(&mut self);
}

impl TreeKillChild {
    pub(crate) fn new(child: tokio::process::Child) -> Self {
        let tree = platform_tree(&child);
        Self {
            child,
            tree,
            given_up: false,
        }
    }

    /// A guard over `child` whose tree is `tree`, for a case that
    /// decides what the tree does.
    #[cfg(test)]
    pub(crate) fn with_tree(child: tokio::process::Child, tree: Box<dyn Tree>) -> Self {
        Self {
            child,
            tree: Some(tree),
            given_up: false,
        }
    }

    /// The guarded child, for the caller's own I/O and wait.
    pub(crate) fn child_mut(&mut self) -> &mut tokio::process::Child {
        &mut self.child
    }

    /// Whether anything of the tree is still running, for a case.
    #[cfg(all(test, windows))]
    pub(crate) fn tree_running(&mut self) -> bool {
        self.tree.as_mut().is_some_and(|t| t.running())
    }

    /// Take the tree down and wait until it has exited: the child
    /// reaped and, of the rest of its tree, nothing left — which is
    /// also what it checks first, so a tree already gone is not
    /// signalled. `false` when the ceiling passed first: the kill was
    /// asked for, but the tree cannot be said to be gone, and nothing
    /// that would meet it should start.
    pub(crate) async fn take_down(&mut self) -> bool {
        if self.tree.is_none() {
            return true;
        }
        if self.tree_exited() {
            self.tree = None;
            return true;
        }
        self.kill_tree();
        let deadline = std::time::Instant::now() + TREE_EXIT_CEILING;
        loop {
            if self.tree_exited() {
                self.tree = None;
                return true;
            }
            if std::time::Instant::now() >= deadline {
                tracing::warn!(pid = ?self.child.id(), "a tool's process tree outlived its teardown");
                self.given_up = true;
                return false;
            }
            tokio::time::sleep(TREE_EXIT_POLL).await;
        }
    }

    /// [`Self::take_down`] for the drop, which cannot await: the same
    /// wait, sleeping the thread. A teardown's exit takes milliseconds.
    fn take_down_blocking(&mut self) {
        if self.tree.is_none() {
            return;
        }
        if self.given_up {
            self.kill_tree();
            return;
        }
        if self.tree_exited() {
            return;
        }
        self.kill_tree();
        let deadline = std::time::Instant::now() + TREE_EXIT_CEILING;
        while !self.tree_exited() {
            if std::time::Instant::now() >= deadline {
                tracing::warn!(pid = ?self.child.id(), "a tool's process tree outlived its teardown");
                return;
            }
            std::thread::sleep(TREE_EXIT_POLL);
        }
    }

    /// The child first, reaping it if it has exited, then the rest of
    /// its tree.
    fn tree_exited(&mut self) -> bool {
        if matches!(self.child.try_wait(), Ok(None)) {
            return false;
        }
        !self.tree.as_mut().is_some_and(|t| t.running())
    }

    fn kill_tree(&mut self) {
        if let Some(tree) = self.tree.as_mut() {
            tree.kill();
        }
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

/// The tree of a child spawned in a process group of its own: the
/// group, whose id is the child's pid. A group with a process still
/// in it keeps its id from being handed out again, so it can be
/// looked at and signalled past the child's reap.
#[cfg(unix)]
struct ProcessGroup {
    pgid: u32,
}

#[cfg(unix)]
impl Tree for ProcessGroup {
    fn running(&mut self) -> bool {
        std::process::Command::new("kill")
            .args(["-0", "--", &format!("-{}", self.pgid)])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    fn kill(&mut self) {
        kill_process_tree(self.pgid);
    }
}

#[cfg(unix)]
fn platform_tree(child: &tokio::process::Child) -> Option<Box<dyn Tree>> {
    Some(Box::new(ProcessGroup { pgid: child.id()? }))
}

/// The tree of a child on Windows: the members of a job object the
/// child was put in as it started, which the processes it starts
/// join. The kill is `taskkill /T` on the child, for anything started
/// in the moment before it joined the job, and `taskkill` on each
/// member. Without a job — creating or joining one failed — the tree
/// is what `taskkill /T` reaches through the child while it runs.
#[cfg(windows)]
struct JobTree {
    root: u32,
    job: Option<win32job::Job>,
}

#[cfg(windows)]
impl JobTree {
    fn members(&self) -> Vec<u32> {
        let Some(job) = &self.job else {
            return Vec::new();
        };
        match job.query_process_id_list() {
            Ok(pids) => pids
                .into_iter()
                .filter_map(|p| u32::try_from(p).ok())
                .collect(),
            Err(e) => {
                tracing::warn!(error = %e, "a tool's job could not be read");
                Vec::new()
            }
        }
    }
}

#[cfg(windows)]
impl Tree for JobTree {
    fn running(&mut self) -> bool {
        !self.members().is_empty()
    }

    fn kill(&mut self) {
        kill_process_tree(self.root);
        for pid in self.members() {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .output();
        }
    }
}

#[cfg(windows)]
fn platform_tree(child: &tokio::process::Child) -> Option<Box<dyn Tree>> {
    let root = child.id()?;
    let joined = (|| {
        let mut limits = win32job::ExtendedLimitInfo::new();
        limits.limit_kill_on_job_close();
        let job = win32job::Job::create_with_limit_info(&limits)?;
        let handle = child.raw_handle().ok_or_else(|| {
            win32job::JobError::AssignFailed(std::io::Error::other("the tool has already exited"))
        })?;
        job.assign_process(handle as isize)?;
        Ok::<_, win32job::JobError>(job)
    })();
    let job = match joined {
        Ok(job) => Some(job),
        Err(e) => {
            tracing::warn!(error = %e, "a tool could not be put in a job");
            None
        }
    };
    Some(Box::new(JobTree { root, job }))
}

/// Take down a spawned downloader's whole process tree.
///
/// Shared by the drop guard and by the in-flight stop, so both paths
/// use the same platform command and the same test seam.
pub(crate) fn kill_process_tree(pid: u32) {
    #[cfg(test)]
    if let Some(probe) = tree_kill_probe() {
        let _ = std::process::Command::new(probe)
            .arg(pid.to_string())
            .status();
        return;
    }
    if let Some((prog, args)) = tree_kill_args(pid, cfg!(windows)) {
        let _ = std::process::Command::new(prog).args(&args).output();
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
