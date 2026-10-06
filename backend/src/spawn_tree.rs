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
pub(super) fn platform_tree(child: &tokio::process::Child) -> Option<Box<dyn super::Tree>> {
    Some(Box::new(ProcessGroup { pgid: child.id()? }))
}

/// The tree of a child on Windows: the members of two job objects the
/// child was put in as it started, the inner nested in the outer, both
/// made to end their members when their last handle — the guard's —
/// closes. The processes the child starts join them too — except any
/// it might start in the moment between the two joins, which are in
/// the outer job only. Closing the inner job does not end those; unless
/// the root's tree kill reaches them, the teardown waits to its
/// ceiling and gives up, which ends the download, and the guard's drop
/// then kills each member the outer job still lists and closes it.
/// The joins are microseconds apart.
///
/// The kill closes the inner job: the kernel ends every member at once,
/// and no pid is looked up, so none can have been handed to another
/// program in the meantime. The outer job stays open, so the guard can
/// still ask whether the members have gone; closing it would end them
/// the same way but leave nothing to ask, and the crate has no call
/// that ends a job's members while keeping the job. The outer job also
/// ends them when the backend dies without running its guards. See
/// [`kill_plan`] for the rest of the decision.
#[cfg(windows)]
struct JobTree {
    root: u32,
    outer: Option<win32job::Job>,
    inner: Option<win32job::Job>,
}

#[cfg(windows)]
impl JobTree {
    fn members(&self) -> Vec<u32> {
        let Some(job) = &self.outer else {
            return Vec::new();
        };
        match job.query_process_id_list() {
            Ok(pids) => pids
                .into_iter()
                .filter_map(|p| u32::try_from(p).ok())
                .collect(),
            Err(e) => {
                tracing::warn!(error = ?e, "a tool's job could not be read");
                Vec::new()
            }
        }
    }
}

#[cfg(windows)]
impl super::Tree for JobTree {
    fn running(&mut self) -> bool {
        !self.members().is_empty()
    }

    #[cfg(test)]
    fn kills_at_once(&self) -> bool {
        self.inner.is_some()
    }

    fn kill(&mut self) {
        let members = self.members();
        let listed = self.outer.as_ref().map(|_| members.as_slice());
        let plan = kill_plan(listed, self.root, self.inner.is_some());
        if plan.tree_kill_root {
            super::kill_process_tree(self.root);
        }
        if plan.close_inner {
            drop(self.inner.take());
        }
        for pid in plan.each_member {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .output();
        }
    }
}

/// What a Windows teardown does to a tool's tree.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct KillPlan {
    /// `taskkill /T` to the root, for anything it started before it
    /// joined its jobs — see [`kills_root_tree`].
    pub(super) tree_kill_root: bool,
    /// Close the inner job, which ends every member at once.
    pub(super) close_inner: bool,
    /// `taskkill /F` to each of these. Only without an inner job to
    /// close — never made, or closed by an earlier kill whose members
    /// outlived the wait — and only what the outer job lists then.
    pub(super) each_member: Vec<u32>,
}

/// The Windows teardown's decision, from the outer job's members
/// (`None` with no job), the root's pid and whether the inner job is
/// still open.
#[cfg_attr(not(windows), allow(dead_code))]
pub(super) fn kill_plan(members: Option<&[u32]>, root: u32, inner_open: bool) -> KillPlan {
    KillPlan {
        tree_kill_root: kills_root_tree(members, root),
        close_inner: inner_open,
        each_member: if inner_open {
            Vec::new()
        } else {
            members.unwrap_or_default().to_vec()
        },
    }
}

/// Whether a Windows teardown sends `taskkill /T` to the root: only
/// while the root is still running, so its pid is still its own. With
/// a job (`members` is its member list), that is while the root is a
/// member — it leaves the list as it exits, and after that its pid may
/// belong to another program, whose tree `/T` would take down. With no
/// job, the guard only asks for a kill while it holds the child
/// unreaped, so the root is alive.
#[cfg_attr(not(windows), allow(dead_code))]
pub(super) fn kills_root_tree(members: Option<&[u32]>, root: u32) -> bool {
    members.is_none_or(|m| m.contains(&root))
}

/// A job that ends its members when its last handle closes, with the
/// child in it. Joining a second job nests it in the first.
#[cfg(windows)]
fn job_with(child: &tokio::process::Child) -> Result<win32job::Job, win32job::JobError> {
    let mut limits = win32job::ExtendedLimitInfo::new();
    limits.limit_kill_on_job_close();
    let job = win32job::Job::create_with_limit_info(&limits)?;
    let handle = child.raw_handle().ok_or_else(|| {
        win32job::JobError::AssignFailed(std::io::Error::other("the tool has already exited"))
    })?;
    job.assign_process(handle as isize)?;
    Ok(job)
}

#[cfg(windows)]
pub(super) fn platform_tree(child: &tokio::process::Child) -> Option<Box<dyn super::Tree>> {
    let root = child.id()?;
    let outer = job_with(child)
        .map_err(|e| tracing::warn!(error = ?e, "a tool could not be put in a job"))
        .ok();
    let inner = outer.as_ref().and_then(|_| {
        job_with(child)
            .map_err(|e| tracing::warn!(error = ?e, "a tool could not be put in its inner job"))
            .ok()
    });
    Some(Box::new(JobTree { root, outer, inner }))
}
