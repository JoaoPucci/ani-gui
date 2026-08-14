//! Removal of the files earlier versions left behind.
//!
//! Until 0.11 the app kept its own copy of the shell script under the
//! cache root, refreshed it on launch, and appended the outcome of
//! every run to a log under the state dir that the diagnostics page
//! read back. Playback, downloads and availability all resolve
//! natively now, and none of that machinery survives — so on every
//! machine that ever ran one of those versions both files sit there:
//! written by the app, never read again, with nothing left that would
//! remove them.
//!
//! The sweep runs at boot and reports what it removed, so the removal
//! is visible in diagnostics rather than silent.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// What one sweep removed. Empty on every launch after the first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SweepReport {
    /// Paths actually deleted, in the order they were removed.
    pub removed: Vec<PathBuf>,
}

/// Delete whatever the retired script machinery left in `cache_dir`
/// and `state_dir`, and report it.
///
/// Named by exact path rather than by pattern. Both directories hold
/// live state — the image cache and the metadata database in one, the
/// watch history and the account tokens in the other — so anything
/// broader than an explicit list risks taking a file that matters.
///
/// Never fails the caller: this runs during boot, and a directory that
/// cannot be read or written is not a reason to refuse to start.
#[must_use]
pub fn sweep_legacy_files(cache_dir: &Path, state_dir: &Path) -> SweepReport {
    let mut removed = Vec::new();
    for target in [
        cache_dir.join(SCRIPT_NAME),
        state_dir.join(UPDATE_LOG_NAME),
        state_dir.join(format!("{UPDATE_LOG_NAME}.new")),
    ] {
        if remove_if_present(&target) {
            removed.push(target);
        }
    }
    removed.extend(sweep_staging_copies(cache_dir));
    SweepReport { removed }
}

/// Remove the per-pid staging copies an interrupted update left behind.
///
/// The updater wrote one of these beside the script before going to
/// the network and deleted it when the run ended, so a process killed
/// mid-update leaves one per launch — and the code that would have
/// cleaned them up is gone.
///
/// The one place this sweep matches on a pattern rather than an exact
/// name, because the pid is unknowable from here. The pattern is as
/// tight as the thing it is cleaning up: the staging prefix followed
/// by digits, which is all the updater ever wrote. A user's own
/// `ani-cli.bak`, or an `ani-cli.update-staging.notes` nobody's code
/// produced, is not ours to delete.
fn sweep_staging_copies(cache_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return Vec::new();
    };
    let mut removed: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_prefix(STAGING_PREFIX))
                // The suffix the updater wrote was a process id and
                // nothing else. Requiring digits is what separates a
                // file it made from one that merely wears the name.
                .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
        })
        .filter(|p| remove_if_present(p))
        .collect();
    // `read_dir` order is filesystem-defined; sort so the diagnostics
    // block lists the same thing twice in a row.
    removed.sort();
    removed
}

/// Remove one path, reporting whether it actually went.
fn remove_if_present(target: &Path) -> bool {
    // `is_file` rather than `exists`: a directory wearing one of these
    // names is not what the old code wrote, and removing it is not
    // ours to do. It also covers the fresh-profile case, where the
    // parent directory does not exist yet.
    if !target.is_file() {
        return false;
    }
    match std::fs::remove_file(target) {
        Ok(()) => true,
        // A directory we cannot write is not a reason to refuse to
        // boot, and the next launch tries again. Reporting a removal
        // that did not happen would be worse than reporting nothing.
        Err(e) => {
            tracing::warn!(
                target: "legacy_script",
                path = %target.display(),
                error = %e,
                "could not remove a file left by the retired updater"
            );
            false
        }
    }
}

/// The script copy the updater maintained under the cache root.
const SCRIPT_NAME: &str = "ani-cli";

/// The outcome log the updater appended to under the state dir. Written
/// `.new`-then-rename, so the temporary is swept alongside it.
const UPDATE_LOG_NAME: &str = "anicli-update-log.json";

/// Prefix of the staging copies the updater made beside the script,
/// one per launch, each suffixed with the process id.
const STAGING_PREFIX: &str = "ani-cli.update-staging.";

#[cfg(test)]
#[path = "legacy_script_test.rs"]
mod tests;
