//! Removal of the bundled `ani-cli` copy earlier versions maintained.
//!
//! Until 0.11 the app kept its own copy of the script under the cache
//! root and refreshed it on launch. Nothing reads it now — playback,
//! downloads and availability all resolve natively — so it is left
//! behind on every machine that ever ran one of those versions: a file
//! the app wrote, will never touch again, and would otherwise leave
//! sitting there indefinitely.
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

/// Delete the maintained copy of the script from `cache_dir`, if one
/// is there, and report it.
///
/// Never fails the caller: this runs during boot, and a cache that
/// cannot be read or written is not a reason to refuse to start.
#[must_use]
pub fn sweep_legacy_script(cache_dir: &Path) -> SweepReport {
    let target = cache_dir.join(SCRIPT_NAME);
    // `is_file` rather than `exists`: a directory wearing this name is
    // not the file the updater wrote, and removing it is not ours to
    // do. It also covers the fresh-profile case, where the cache root
    // does not exist yet.
    if !target.is_file() {
        return SweepReport::default();
    }
    match std::fs::remove_file(&target) {
        Ok(()) => SweepReport {
            removed: vec![target],
        },
        // A cache we cannot write is not a reason to refuse to boot,
        // and the next launch tries again. Reporting a removal that
        // did not happen would be worse than reporting nothing.
        Err(e) => {
            tracing::warn!(
                target: "legacy_script",
                path = %target.display(),
                error = %e,
                "could not remove the orphaned script copy"
            );
            SweepReport::default()
        }
    }
}

/// The name the updater wrote under the cache root.
const SCRIPT_NAME: &str = "ani-cli";

#[cfg(test)]
#[path = "legacy_script_test.rs"]
mod tests;
