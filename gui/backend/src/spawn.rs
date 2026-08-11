//! Running an external tool and reading what it prints.
//!
//! Process-group lifecycle and output cleaning for the binaries this
//! backend spawns — the downloader's yt-dlp and ffmpeg, and (once the
//! script retires) the external player and Syncplay. None of it is
//! ani-cli's: it lived under `anicli/` only because that is where the
//! first subprocess driver was written, and the native paths reached
//! across the module boundary to reuse it.

/// Owns the spawned ani-cli child and kills its whole process tree
/// when dropped mid-run. Ownership is the point: a struct's `Drop`
/// body runs BEFORE its fields drop, so the tree walk / group signal
/// fires while the shell is still alive — on Windows `taskkill /T`
/// can only discover curl / yt-dlp / ffmpeg descendants by a live
/// parent pid, and `kill_on_drop`'s SIGKILL (the `Child` field's own
/// drop) must come second under every cancellation mode: task abort,
/// timeout, panic. Disarmed once the child has been waited: past the
/// reap the pid may be recycled and must not be signalled. The signal
/// goes through `kill(1)` / `taskkill(1)` rather than a syscall —
/// the crate forbids unsafe code, and both binaries ship with any
/// host that can run ani-cli. `.status()` (not `.spawn()`) so the
/// helper can't linger as a zombie; it exits in microseconds.
pub(crate) struct TreeKillChild {
    pub(crate) child: tokio::process::Child,
    armed: bool,
}

impl TreeKillChild {
    pub(crate) fn new(child: tokio::process::Child) -> Self {
        Self { child, armed: true }
    }

    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }

    /// The guarded child, for the caller's own I/O and wait. A child
    /// the caller has waited to completion is reaped, so the drop
    /// guard reads `id() == None` and stands down by itself.
    pub(crate) fn child_mut(&mut self) -> &mut tokio::process::Child {
        &mut self.child
    }
}

impl Drop for TreeKillChild {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // `id()` is None once the child has been reaped — nothing to
        // walk in that case even if disarm was missed.
        let Some(pid) = self.child.id() else { return };
        kill_process_tree(pid);
    }
}

/// Take down a spawned downloader's whole process tree.
///
/// Shared by the drop guard and by the in-flight stop below, so both
/// paths use the same platform command and the same test seam.
pub(crate) fn kill_process_tree(pid: u32) {
    #[cfg(test)]
    if let Some(probe) = tree_kill_probe() {
        let _ = std::process::Command::new(probe)
            .arg(pid.to_string())
            .status();
        return;
    }
    if let Some((prog, args)) = tree_kill_args(pid, cfg!(windows)) {
        let _ = std::process::Command::new(prog).args(&args).status();
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
