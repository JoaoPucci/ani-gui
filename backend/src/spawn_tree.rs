//! A spawned tool's tree, per platform: what the guard in
//! [`super::guard`] looks at and kills beyond the tool itself.

/// The tree of a child spawned in a process group of its own: the
/// group, whose id is the child's pid. A group with a process still
/// in it keeps its id from being handed out again, so it can be
/// looked at and signalled past the child's reap.
#[cfg(unix)]
struct ProcessGroup {
    pgid: u32,
}

#[cfg(unix)]
impl super::Tree for ProcessGroup {
    fn running(&mut self) -> bool {
        still_running(
            std::process::Command::new("kill")
                .args(["-0", "--", &format!("-{}", self.pgid)])
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success()),
        )
    }

    fn kill(&mut self) {
        super::kill_process_tree(self.pgid);
    }
}

#[cfg(unix)]
pub(super) fn process_group(child: &tokio::process::Child) -> Option<Box<dyn super::Tree>> {
    Some(Box::new(ProcessGroup { pgid: child.id()? }))
}

/// The tree of a child on Windows: the members of a job object the
/// child was put in before it ran, made to end its members when its
/// last handle — the guard's — closes, so a backend that dies without
/// running its guards takes its tools with it. The processes the child
/// starts join it, and it lists them whether or not the child itself
/// still runs.
///
/// The kill is not here: the child also runs in a job of its own,
/// nested in this one, which the guard ends through the child — every
/// member at once, with no pid looked up. This job is the one that can
/// still be asked whether they have gone. Without it — it could not be
/// made — the guard still ends the tool's own job at every teardown,
/// and that job ends its members when it closes, but the guard can
/// only see the child itself go: a teardown then returns once the kill
/// is asked for and the child has exited, without waiting on the rest
/// of the tree to exit as it does everywhere else.
#[cfg(windows)]
struct JobTree {
    job: Option<win32job::Job>,
    /// A failed read of the job was logged: a teardown reads it every
    /// few milliseconds, so it is logged once per tree.
    warned: bool,
}

#[cfg(windows)]
impl super::Tree for JobTree {
    fn running(&mut self) -> bool {
        let Some(job) = &self.job else {
            return false;
        };
        let look = job.query_process_id_list().map(|pids| !pids.is_empty());
        if let Err(e) = &look {
            if !self.warned {
                tracing::warn!(error = ?e, "a tool's job could not be read");
                self.warned = true;
            }
        }
        still_running(look)
    }

    fn kill(&mut self) {}
}

#[cfg(windows)]
pub(super) fn job_tree(job: Option<win32job::Job>) -> Option<Box<dyn super::Tree>> {
    Some(Box::new(JobTree { job, warned: false }))
}

/// Whether a look at a tree found it still running: what the look
/// answered, or, when the look could not be made, running — a failed
/// look says nothing about the tree, and only a tree seen gone ends a
/// teardown's wait.
fn still_running<E>(look: Result<bool, E>) -> bool {
    look.unwrap_or(true)
}

#[cfg(test)]
#[path = "spawn_tree_test.rs"]
mod tests;
