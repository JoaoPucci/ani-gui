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
pub fn sweep_legacy_script(_cache_dir: &Path) -> SweepReport {
    todo!("sweep the orphaned script copy")
}

#[cfg(test)]
#[path = "legacy_script_test.rs"]
mod tests;
