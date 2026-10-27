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
pub(crate) use guard::{Tree, TreeKillChild};
#[cfg(test)]
pub(crate) use guard::{TREE_EXIT_CEILING, TREE_KILL_REPEAT};

/// Take down a spawned downloader's whole process group, on Unix.
///
/// Called in the app only by the process group's kill, the one way a
/// guard's teardown — awaited or dropped, whatever ended the run, a
/// download's in-flight stop included — signals a tree on Unix, so
/// every teardown uses the same command and the same test seam. On Windows the tree ends through its job instead; see
/// [`GuardedCommand`].
#[cfg(unix)]
pub(crate) fn kill_process_tree(pid: u32) {
    #[cfg(test)]
    {
        // Held for the probe's whole run, not only the read: the case
        // that registered it withdraws it and removes the script as it
        // ends, and a script gone between this read and its exec is a
        // kill nobody sends.
        let probe = TREE_KILL_PROBE.lock().expect("probe lock");
        if let Some(probe) = probe.as_deref() {
            run_probe(probe, pid);
            return;
        }
    }
    let (prog, args) = tree_kill_args(pid);
    let _ = std::process::Command::new(prog).args(&args).output();
}

/// Run the registered probe with `pid`, waiting out a busy script. A
/// case stages its probe a moment before a teardown runs it, and a
/// fork elsewhere in the test binary in that moment holds the script's
/// write descriptor until the child execs: the exec is refused as busy
/// (ETXTBSY), which is transient by construction. Taken for done, it
/// would be a kill nobody sends. Retried until it runs, under a
/// deadline long enough for any machine to get there: one counted in
/// a few short retries would give up on a slow machine for nothing.
#[cfg(all(test, unix))]
fn run_probe(probe: &std::path::Path, pid: u32) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        match std::process::Command::new(probe)
            .arg(pid.to_string())
            .status()
        {
            Err(e)
                if e.kind() == std::io::ErrorKind::ExecutableFileBusy
                    && std::time::Instant::now() < deadline =>
            {
                // Busy: say so to a case holding it, which can then let
                // it go knowing the retry is what it tests.
                crate::held::queued(&probe.display().to_string());
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            _ => return,
        }
    }
}

/// Test seam: when a probe is registered, the teardown runs it (child
/// pid as its argument) INSTEAD of the real tree-kill command, so a
/// case can decide when a kill lands — later than asked, say. The
/// probe takes over the cleanup duty too. A teardown holds this lock
/// while the probe runs, so withdrawing it waits for any run in
/// flight to end.
#[cfg(all(test, unix))]
pub(crate) static TREE_KILL_PROBE: std::sync::Mutex<Option<std::path::PathBuf>> =
    std::sync::Mutex::new(None);

/// Serializes the probe's scope: a test that REGISTERS a probe (and
/// so redirects every teardown in the process) and a test that needs
/// the REAL teardown to run must not overlap — a no-op probe held by
/// one would silently swallow the other's kill. Held for the whole
/// test either way. Most cases with a real teardown do not take it,
/// so a registered probe serves their trees too: it may delay another
/// case's kill only well inside [`TREE_EXIT_CEILING`], and may withhold
/// a kill or stretch it past the ceiling only for its own case's trees.
#[cfg(all(test, unix))]
pub(crate) static TREE_KILL_PROBE_SCOPE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
