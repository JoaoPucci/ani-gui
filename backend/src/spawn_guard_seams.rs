//! The guard's ceiling and the seams cases reach it through — split
//! from `spawn_guard` for the per-file complexity bar.

use super::TreeKillChild;

impl TreeKillChild {
    /// The guard with its teardowns waiting up to `ceiling` instead.
    /// For a case that asserts what a teardown does rather than how
    /// long it may take: the app's ceiling is a bound a loaded test
    /// runner can stall past with nothing wrong, so such a case waits
    /// under a failure bound of its own.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_ceiling(mut self, ceiling: std::time::Duration) -> Self {
        self.ceiling = ceiling;
        self
    }

    /// The guard with an awaited teardown's ceiling measured on tokio's
    /// clock, the one its waits between looks sleep on. On a paused
    /// clock, time then moves only from one of those waits to the next,
    /// so when the teardown gives up does not depend on how fast the
    /// machine runs the looks. Only for a case: a paused clock moves on
    /// a wait at once, so a real tree's teardown on it would give up
    /// after so many looks rather than so much time.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn on_tokio_clock(mut self) -> Self {
        self.tokio_clock = true;
        self
    }

    /// Whether an awaited teardown begun at `started` (on the wall
    /// clock, and on tokio's) is past its ceiling.
    pub(super) fn past_ceiling(&self, started: (std::time::Instant, tokio::time::Instant)) -> bool {
        #[cfg(test)]
        if self.tokio_clock {
            return started.1.elapsed() >= self.ceiling;
        }
        started.0.elapsed() >= self.ceiling
    }

    /// A guard over `child` whose tree is `tree`, for a case that
    /// decides what the tree does.
    #[cfg(test)]
    pub(crate) fn with_tree(child: tokio::process::Child, tree: Box<dyn super::Tree>) -> Self {
        #[cfg(windows)]
        let child: super::GuardedChild = Box::new(child);
        Self::guard(child, Some(tree))
    }

    /// Whether anything of the tree is still running, for a case.
    #[cfg(test)]
    pub(crate) fn tree_running(&mut self) -> bool {
        self.tree.as_mut().is_some_and(|t| t.running())
    }
}
