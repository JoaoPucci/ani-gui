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
        std::process::Command::new("kill")
            .args(["-0", "--", &format!("-{}", self.pgid)])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
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
/// only see the child itself go.
#[cfg(windows)]
struct JobTree {
    job: Option<win32job::Job>,
}

#[cfg(windows)]
impl super::Tree for JobTree {
    fn running(&mut self) -> bool {
        let Some(job) = &self.job else {
            return false;
        };
        match job.query_process_id_list() {
            Ok(pids) => !pids.is_empty(),
            Err(e) => {
                tracing::warn!(error = ?e, "a tool's job could not be read");
                false
            }
        }
    }

    fn kill(&mut self) {}
}

#[cfg(windows)]
pub(super) fn job_tree(job: Option<win32job::Job>) -> Option<Box<dyn super::Tree>> {
    Some(Box::new(JobTree { job }))
}
