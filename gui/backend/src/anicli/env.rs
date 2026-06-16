//! PATH composition for `ani-cli` subprocess spawns.
//!
//! On Windows we ship a `bin/` directory next to the backend binary
//! containing `fzf.exe` (and any future POSIX-side ani-cli deps that
//! Git for Windows doesn't bundle). The script's `command -v fzf`
//! must resolve to that bundled copy before the system PATH, so we
//! prepend the bundled dir at every spawn site.
//!
//! This module exposes a single pure function that the spawn sites
//! call instead of building the PATH string inline. Pure (no env or
//! filesystem reads) so tests can drive every branch deterministically.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::error::{AniError, Result};

/// Default PATH used when neither the inherited env nor a test
/// override provides one. Matches the previous inline literal in
/// `process.rs` so behaviour is unchanged on a freshly-cleared env.
const FALLBACK_PATH: &str = "/usr/bin:/bin";

/// Compose the PATH env var for an ani-cli spawn.
///
/// Order of components in the returned value (platform-correct
/// separator via [`std::env::join_paths`]):
///
/// 1. `bundled_bin` — if provided, prepended so the bundled fzf wins
///    over any system install.
/// 2. `path_override` — wins over the inherited PATH when set
///    (tests inject this to put a curl shim ahead of system bins).
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

/// Names of OS env vars the ani-cli spawn must forward on Windows
/// after `cmd.env_clear()`. Without these, Git Bash can't bootstrap
/// its MSYS mount table (so `/tmp` resolves to a path the user often
/// can't write — see the cascade of `mktemp: ... Permission denied`
/// followed by empty-variable bash errors that turned a regular
/// click-to-play into a generic "Network trouble" toast).
///
/// Inert on Unix: kept here so `windows_env_passthrough` is callable
/// from cross-platform unit tests, but the spawn-site call is
/// `#[cfg(windows)]`-gated so Linux runs are byte-identical to today.
///
/// Order is stable so callers can rely on it for deterministic env
/// snapshots in tests.
pub const WINDOWS_ENV_PASSTHROUGH_KEYS: &[&str] = &[
    "TMP",
    "TEMP",
    "SYSTEMROOT",
    "USERPROFILE",
    "LOCALAPPDATA",
    "APPDATA",
    "COMSPEC",
    "WINDIR",
];

/// Windows env-var passthrough for the ani-cli spawn. Pure with
/// respect to `read`, which the caller injects: production calls pass
/// `|k| std::env::var_os(k)`; tests pass a closure backed by a
/// `HashMap` so they pin exact behaviour without touching real env.
///
/// Returns the (name, value) pairs to apply with `cmd.env(name, value)`
/// after `cmd.env_clear()`. Only entries whose values are present
/// (i.e. `read` returned `Some(_)`) are emitted, in the order defined
/// by [`WINDOWS_ENV_PASSTHROUGH_KEYS`]. Empty values are forwarded
/// (Windows env API treats empty string as "set"; Git Bash distinguishes
/// it from missing).
#[must_use]
pub fn windows_env_passthrough(
    read: impl Fn(&str) -> Option<OsString>,
) -> Vec<(&'static str, OsString)> {
    WINDOWS_ENV_PASSTHROUGH_KEYS
        .iter()
        .filter_map(|k| read(k).map(|v| (*k, v)))
        .collect()
}

/// Locate `ffmpeg` inside a composed PATH string. Pure: caller
/// supplies the path-list and the executable check, so the test
/// suite can drive every branch without touching real disk.
///
/// Returns `Ok(())` when an executable matching the platform's
/// ffmpeg name (`ffmpeg.exe` on Windows, `ffmpeg` elsewhere) sits
/// in any of the path components. Otherwise returns
/// [`AniError::FfmpegMissing`] so the SSE download stream can
/// short-circuit before spawning ani-cli — surfacing the typed
/// error early lets the frontend render a clear modal instead of
/// the generic "Download failed" the post-spawn dep_ch failure
/// otherwise produces.
pub fn ensure_ffmpeg_in_path(
    composed_path: &OsStr,
    is_executable: impl Fn(&Path) -> bool,
) -> Result<()> {
    // Platform-correct binary name: Windows resolves bare names by
    // appending PATHEXT, but our caller (the bash subprocess on
    // Windows) walks PATH literally and only matches `ffmpeg.exe`.
    // Match that behaviour exactly so the pre-check agrees with
    // what the spawn would see.
    let exe_name: &str = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    for dir in std::env::split_paths(composed_path) {
        // split_paths on Unix yields a single empty PathBuf for an
        // empty input — that path joins to bare "ffmpeg" which would
        // false-positive in any callback that accepts every path.
        // bash's command -v likewise ignores empty PATH components.
        if dir.as_os_str().is_empty() {
            continue;
        }
        if is_executable(&dir.join(exe_name)) {
            return Ok(());
        }
    }
    Err(AniError::FfmpegMissing)
}

#[cfg(test)]
#[path = "env_test.rs"]
mod tests;
