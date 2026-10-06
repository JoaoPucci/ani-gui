//! Running an external tool and reading what it prints.
//!
//! A spawned tool's tree and its teardown — a process group on Unix,
//! job objects on Windows — and output cleaning for the binaries this
//! backend spawns: the downloader's yt-dlp and ffmpeg, the external
//! player, and Syncplay. It lived under `anicli/` only because the
//! first subprocess driver was written there, and the native paths
//! reached across the module boundary to reuse it.

#[path = "spawn_command.rs"]
mod command;
#[path = "spawn_guard.rs"]
mod guard;
#[path = "spawn_tree.rs"]
mod tree;

pub(crate) use command::GuardedCommand;
#[cfg(test)]
pub(crate) use guard::TREE_EXIT_CEILING;
pub(crate) use guard::{Tree, TreeKillChild};

/// Take down a spawned downloader's whole process group, on Unix.
///
/// Shared by the drop guard and by the in-flight stop, so both paths
/// use the same command and the same test seam. On Windows the tree
/// ends through its job instead; see [`GuardedCommand`].
#[cfg(unix)]
pub(crate) fn kill_process_tree(pid: u32) {
    #[cfg(test)]
    if let Some(probe) = tree_kill_probe() {
        let _ = std::process::Command::new(probe)
            .arg(pid.to_string())
            .status();
        return;
    }
    let (prog, args) = tree_kill_args(pid);
    let _ = std::process::Command::new(prog).args(&args).output();
}

/// Test seam: when a probe is registered, the teardown runs it (child
/// pid as its argument) INSTEAD of the real tree-kill command, so a
/// case can decide when a kill lands — later than asked, say. The
/// probe takes over the cleanup duty too.
#[cfg(all(test, unix))]
pub(crate) static TREE_KILL_PROBE: std::sync::Mutex<Option<std::path::PathBuf>> =
    std::sync::Mutex::new(None);

/// Serializes the probe's scope: a test that REGISTERS a probe (and
/// so redirects every teardown in the process) and a test that needs
/// the REAL teardown to run must not overlap — a no-op probe held by
/// one would silently swallow the other's kill. Held for the whole
/// test either way.
#[cfg(all(test, unix))]
pub(crate) static TREE_KILL_PROBE_SCOPE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(all(test, unix))]
fn tree_kill_probe() -> Option<std::path::PathBuf> {
    TREE_KILL_PROBE.lock().expect("probe lock").clone()
}

/// The command that takes down a spawned downloader's process group:
/// `kill -9 -- -PID` — the negative pid addresses the group created at
/// spawn (pgid == child pid).
#[cfg(unix)]
fn tree_kill_args(pid: u32) -> (&'static str, Vec<String>) {
    ("kill", vec!["-9".into(), "--".into(), format!("-{pid}")])
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
