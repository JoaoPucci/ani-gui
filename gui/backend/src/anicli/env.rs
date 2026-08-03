//! PATH composition for the `ani-cli` auto-updater's spawn.
//!
//! On Windows we ship a `bin/` directory next to the backend binary
//! containing POSIX-side deps Git for Windows doesn't bundle. The
//! updater's `command -v` lookups must resolve the bundled copies
//! before the system PATH, so the spawn site prepends the bundled
//! dir through this one pure function. Pure (no env or filesystem
//! reads) so tests can drive every branch deterministically.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Default PATH used when neither the inherited env nor a test
/// override provides one, so behaviour on a freshly-cleared env is
/// deterministic.
const FALLBACK_PATH: &str = "/usr/bin:/bin";

/// Compose the PATH env var for the updater's script spawn.
///
/// Order of components in the returned value (platform-correct
/// separator via [`std::env::join_paths`]):
///
/// 1. `bundled_bin` — if provided, prepended so bundled deps win
///    over any system install.
/// 2. `path_override` — wins over the inherited PATH when set.
/// 3. `inherited` — the parent process's PATH, normally
///    `std::env::var_os("PATH")`.
/// 4. [`FALLBACK_PATH`] — last-ditch when none of the above are set.
///
/// Pure: no env or filesystem reads. Caller passes everything in.
#[must_use]
pub fn compose_anicli_path(
    bundled_bin: Option<&Path>,
    path_override: Option<&str>,
    inherited: Option<&OsStr>,
) -> OsString {
    let base: OsString = match path_override {
        Some(o) => OsString::from(o),
        None => match inherited {
            Some(p) => p.to_os_string(),
            None => OsString::from(FALLBACK_PATH),
        },
    };

    let mut parts: Vec<PathBuf> = Vec::new();
    if let Some(b) = bundled_bin {
        parts.push(b.to_path_buf());
    }
    for p in std::env::split_paths(&base) {
        parts.push(p);
    }

    // join_paths only fails if a component contains the platform's
    // path-list separator, which neither our bundled dir nor a
    // pre-split PATH should ever contain. Fall back to the un-prefixed
    // base string so a malformed bundled dir doesn't break spawns.
    std::env::join_paths(&parts).unwrap_or(base)
}

#[cfg(test)]
#[path = "env_test.rs"]
mod tests;
