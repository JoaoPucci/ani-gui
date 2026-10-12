//! Spawning a tool under its guard, with the tool in its tree before
//! it runs.
//!
//! A tool that starts a helper the moment it runs must already be in
//! the tree the guard looks after, or the helper is outside it. On
//! Unix the tool's process group is set between fork and exec. On
//! Windows a process runs from the moment it is created, so the tool
//! is created suspended, put in its jobs, and only then resumed —
//! through `process-wrap`, whose job wrapper does exactly that, so the
//! crate's ban on unsafe code holds. Its job is the one the guard ends
//! the tree through; the job this module adds before it, through
//! `win32job`, is the one the guard asks whether the tree has gone,
//! and the one whose closing ends the tree with the backend.

use super::TreeKillChild;

/// A command to spawn under a [`TreeKillChild`].
pub(crate) struct GuardedCommand {
    #[cfg(unix)]
    cmd: tokio::process::Command,
    #[cfg(windows)]
    wrap: process_wrap::tokio::CommandWrap,
    #[cfg(windows)]
    outer: std::sync::Arc<std::sync::Mutex<Option<win32job::Job>>>,
}

impl GuardedCommand {
    pub(crate) fn new(cmd: tokio::process::Command) -> Self {
        #[cfg(unix)]
        {
            let mut cmd = cmd;
            cmd.process_group(0);
            Self { cmd }
        }
        #[cfg(windows)]
        {
            let outer = std::sync::Arc::default();
            let mut wrap = process_wrap::tokio::CommandWrap::from(cmd);
            // `JobObject` creates the tool suspended and resumes it once
            // every post-spawn hook has run, the outer job's among them;
            // `KillOnDrop` makes the tool's own job end its members when
            // its handle closes.
            wrap.wrap(OuterJob(std::sync::Arc::clone(&outer)))
                .wrap(process_wrap::tokio::KillOnDrop)
                .wrap(process_wrap::tokio::JobObject);
            Self { wrap, outer }
        }
    }

    /// Whether the command carries the wrapper `W`, for a case.
    #[cfg(all(test, windows))]
    pub(crate) fn wraps<W: process_wrap::tokio::CommandWrapper + 'static>(&self) -> bool {
        self.wrap.has_wrap::<W>()
    }

    /// Whether the command carries the outer job's wrapper, for a case.
    #[cfg(all(test, windows))]
    pub(crate) fn wraps_outer_job(&self) -> bool {
        self.wrap.has_wrap::<OuterJob>()
    }

    /// Spawn the tool, in its tree before it runs, under its guard.
    pub(crate) fn spawn(&mut self) -> std::io::Result<TreeKillChild> {
        #[cfg(unix)]
        {
            let child = self.cmd.spawn()?;
            let tree = super::tree::process_group(&child);
            Ok(TreeKillChild::guard(child, tree))
        }
        #[cfg(windows)]
        {
            let child = self.wrap.spawn()?;
            let job = self.outer.lock().expect("outer job slot").take();
            Ok(TreeKillChild::guard(child, super::tree::job_tree(job)))
        }
    }
}

/// Puts the tool, still suspended, in a job that ends its members when
/// its handle closes. Runs after the spawn and before `JobObject`
/// resumes the tool, whose own job then nests in this one.
#[cfg(windows)]
struct OuterJob(std::sync::Arc<std::sync::Mutex<Option<win32job::Job>>>);

#[cfg(windows)]
impl std::fmt::Debug for OuterJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OuterJob")
    }
}

#[cfg(windows)]
impl process_wrap::tokio::CommandWrapper for OuterJob {
    fn post_spawn(
        &mut self,
        _command: &mut tokio::process::Command,
        child: &mut tokio::process::Child,
        _core: &process_wrap::tokio::CommandWrap,
    ) -> std::io::Result<()> {
        let joined = (|| {
            let mut limits = win32job::ExtendedLimitInfo::new();
            limits.limit_kill_on_job_close();
            let job = win32job::Job::create_with_limit_info(&limits)?;
            let handle = child.raw_handle().ok_or_else(|| {
                win32job::JobError::AssignFailed(std::io::Error::other(
                    "the tool has already exited",
                ))
            })?;
            job.assign_process(handle as isize)?;
            Ok::<_, win32job::JobError>(job)
        })();
        match joined {
            Ok(job) => *self.0.lock().expect("outer job slot") = Some(job),
            Err(e) => {
                *self.0.lock().expect("outer job slot") = None;
                tracing::warn!(error = ?e, "a tool could not be put in its outer job");
            }
        }
        Ok(())
    }
}
