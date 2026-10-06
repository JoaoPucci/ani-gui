//! Running an external tool and reading what it prints.
//!
//! Process-group lifecycle and output cleaning for the binaries this
//! backend spawns: the downloader's yt-dlp and ffmpeg, the external
//! player, and Syncplay. It lived under `anicli/` only because the
//! first subprocess driver was written there, and the native paths
//! reached across the module boundary to reuse it.

#[path = "spawn_guard.rs"]
mod guard;
#[path = "spawn_tree.rs"]
mod tree;

#[cfg(test)]
pub(crate) use guard::TREE_EXIT_CEILING;
pub(crate) use guard::{Tree, TreeKillChild};

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
