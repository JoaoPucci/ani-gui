//! The guard a spawned tool runs under: it owns the child, and takes
//! the child's whole tree down — and waits for it to be gone — when
//! the run ends or the guard is dropped. The trees themselves, which
//! differ per platform, are in [`super::tree`].

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
        let tree = super::tree::platform_tree(&child);
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
