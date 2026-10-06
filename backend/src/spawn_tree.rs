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

/// The tree of a child on Windows: the members of a job object the
/// child was put in as it started, which the processes it starts
/// join. The kill is `taskkill` on each member and, while the child
/// itself is still one of them, `taskkill /T` on the child, for
/// anything it started in the moment before it joined the job — see
/// [`kills_root_tree`]. Without a job — creating or joining one
/// failed — the tree is what `taskkill /T` reaches through the child
/// while it runs.
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

    fn kill(&mut self) {
        let members = self.members();
        let members = self.job.as_ref().map(|_| members.as_slice());
        if kills_root_tree(members, self.root) {
            super::kill_process_tree(self.root);
        }
        for pid in members.unwrap_or_default() {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .output();
        }
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

#[cfg(windows)]
pub(super) fn platform_tree(child: &tokio::process::Child) -> Option<Box<dyn super::Tree>> {
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
            tracing::warn!(error = ?e, "a tool could not be put in a job");
            None
        }
    };
    Some(Box::new(JobTree { root, job }))
}
