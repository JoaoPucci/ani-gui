//! Where the vendored `ani-cli` script lives.
//!
//! Nothing in the app spawns the script to resolve a stream any more —
//! that is [`crate::scraper::anidb`]'s job. What remains is locating
//! the copy on disk, which the auto-updater in [`super::update`] needs
//! so it can keep the script current for anyone running it from a
//! terminal alongside the app.

use std::path::PathBuf;

use crate::error::{AniError, Result};

/// Locate the `ani-cli` binary. Looks at `$PATH`, then falls back to a
/// path passed by the caller (typically the Tauri resource directory).
///
/// # Errors
/// Returns [`AniError::MissingBinary`] when no executable is found.
pub fn locate_ani_cli(fallback: Option<&PathBuf>) -> Result<PathBuf> {
    if let Some(found) = find_in_path("ani-cli") {
        return Ok(found);
    }
    if let Some(p) = fallback {
        if p.is_file() {
            return Ok(p.clone());
        }
    }
    Err(AniError::MissingBinary)
}

pub(crate) fn find_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(unix)]
pub(crate) fn is_executable(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111) != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
pub(crate) fn is_executable(p: &std::path::Path) -> bool {
    p.is_file()
        && p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("cmd"))
            .unwrap_or(false)
}

#[cfg(test)]
#[path = "process_test.rs"]
mod tests;
