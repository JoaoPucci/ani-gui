//! The scratch files versions up to 0.18.0 left in a download folder,
//! removed the next time a download runs into it.
//!
//! Those versions wrote a transfer's unfinished files beside the
//! episode, named `.ani-gui-<pid>-<uuid>…`. A download that ended
//! removed them, but a process that never unwinds — a crash, a kill,
//! every quit on Windows, which ends the backend outright — left them,
//! and on Windows a leading dot hides nothing: they sat in the user's
//! folder, often most of an episode each. This version writes nothing
//! of the kind there, so nothing else would ever remove them.
//!
//! Migration code: it goes around v1.0, once installs from before
//! 0.18.1 are no longer expected — see "Retiring the old-scratch
//! sweep" in `docs/deferred-work.md`.
//!
//! The sweep touches only what it can tell is one of them:
//!
//! - a name of exactly that shape ([`name::is_old_scratch`]);
//! - a regular file, never followed through a link — a link or a
//!   directory of that name is left alone;
//! - last written more than [`MIN_AGE`] ago, by this machine's clock.
//!   An older version can still be running beside this one, on the
//!   same folder, and a transfer it is writing is fresh. A time in the
//!   future, or none, is no age at all, and the file stays.
//!
//! A removal that fails — on Windows a file another process still
//! holds open without delete sharing — is logged and the file stays;
//! the next visit tries again. Nothing about it reaches the download,
//! which neither waits for the sweep nor hears how it went.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[path = "download_old_scratch_name.rs"]
mod name;
#[cfg(test)]
pub(crate) use name::is_old_scratch;

/// How long since its last write a leftover must have sat before the
/// sweep takes it for one: past any transfer an older version could
/// still be running.
pub(crate) const MIN_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// The folders one download has swept: each folder it runs into is
/// swept once, however many of its episodes go there.
#[derive(Debug, Default)]
pub(crate) struct OldScratchVisits {
    swept: std::collections::HashSet<PathBuf>,
}

impl OldScratchVisits {
    /// Sweep `dir` on the blocking pool, without waiting for it, unless
    /// this download already has. Migration code, to go around v1.0:
    /// "Retiring the old-scratch sweep" in `docs/deferred-work.md`.
    /// The sweep it started, if it started one; a download drops it.
    pub(crate) fn visit(&mut self, dir: &Path) -> Option<tokio::task::JoinHandle<()>> {
        if !self.first_visit(dir) {
            return None;
        }
        let dir = dir.to_path_buf();
        Some(tokio::task::spawn_blocking(move || {
            sweep(&dir, SystemTime::now());
        }))
    }

    /// Whether this download has not swept `dir` yet, noting that it
    /// now has.
    pub(crate) fn first_visit(&mut self, dir: &Path) -> bool {
        self.swept.insert(dir.to_path_buf())
    }
}

/// Remove, from `dir`, every leftover as the module describes one,
/// aged by `now`.
pub(crate) fn sweep(dir: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        if file_name.to_str().is_some_and(name::is_old_scratch) {
            remove_if_stale(&entry.path(), now);
        }
    }
}

/// Remove `path` if it is a regular file last written more than
/// [`MIN_AGE`] before `now`.
fn remove_if_stale(path: &Path, now: SystemTime) {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return;
    };
    if !meta.file_type().is_file() || !is_stale(meta.modified().ok(), now) {
        return;
    }
    match std::fs::remove_file(path) {
        Ok(()) => tracing::info!(
            path = %path.display(),
            "download: removed an earlier version's unfinished file"
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!(
            path = %path.display(),
            error = %e,
            "download: an earlier version's unfinished file not removed"
        ),
    }
}

/// Whether a file last written at `modified` is older than
/// [`MIN_AGE`] at `now`; never for a time unknown or ahead of `now`.
fn is_stale(modified: Option<SystemTime>, now: SystemTime) -> bool {
    modified
        .and_then(|m| now.duration_since(m).ok())
        .is_some_and(|age| age > MIN_AGE)
}

#[cfg(test)]
#[path = "download_old_scratch_test.rs"]
mod tests;

#[cfg(test)]
#[path = "download_old_scratch_prop_test.rs"]
mod prop_tests;
