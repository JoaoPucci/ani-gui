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

/// Append the botan-shim directory to an already composed PATH.
///
/// Appended — never prepended — so a real Botan installation anywhere
/// on the user's PATH keeps winning `dep_ch_failover`'s lookup; the
/// shim only catches machines that have none. `None` returns the
/// composed PATH untouched. Pure, like [`compose_anicli_path`].
#[must_use]
pub fn append_shim_bin(composed: OsString, shim_bin: Option<&Path>) -> OsString {
    let Some(shim) = shim_bin else {
        return composed;
    };
    let mut parts: Vec<PathBuf> = std::env::split_paths(&composed).collect();
    parts.push(shim.to_path_buf());
    std::env::join_paths(&parts).unwrap_or(composed)
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

/// Decide which tools satisfy the download preflight for the *active*
/// script. The cache copy is not always the bundled 4.15: an existing
/// installation whose auto-update is disabled, failing, or simply not
/// finished yet can still be running a pre-4.15 script whose download
/// mode hard-requires ffmpeg (`dep_ch "ffmpeg" "aria2c"`). Accepting
/// yt-dlp alone against that script would pass the preflight and then
/// die inside the spawn with a generic scraper error — exactly the
/// modal-says-fine-but-download-fails gap the preflight exists to
/// close.
///
/// Returns the platform-correct binary names: yt-dlp and ffmpeg when
/// the script's download dep line is 4.15's
/// `dep_ch_failover "yt-dlp,ffmpeg"`, ffmpeg alone otherwise. Any
/// unrecognized shape (older script, future upstream change, unreadable
/// contents passed as empty) falls back to ffmpeg-only — the
/// conservative direction, since ffmpeg satisfies every script version.
#[must_use]
pub fn download_tool_names(script_contents: &str) -> &'static [&'static str] {
    const BOTH: &[&str] = if cfg!(windows) {
        &["yt-dlp.exe", "ffmpeg.exe"]
    } else {
        &["yt-dlp", "ffmpeg"]
    };
    const FFMPEG_ONLY: &[&str] = if cfg!(windows) {
        &["ffmpeg.exe"]
    } else {
        &["ffmpeg"]
    };
    if download_branch_invokes_failover(script_contents) {
        BOTH
    } else {
        FFMPEG_ONLY
    }
}

/// Whether the script's download dependency branch invokes 4.15's
/// either-tool failover. Only the `download)` case arm that governs
/// `-d` mode speaks for download capability: a customized script may
/// invoke the same failover in an unrelated helper while its download
/// branch still hard-requires ffmpeg. Within the arm the line must
/// BEGIN with the call, as the real script's dep check does —
/// comments, quoted diagnostics, assignments, and a no-op builtin's
/// arguments all grant nothing. The arm opens at a line starting with
/// `download)` (the rest of that line included, for one-liner arms)
/// and closes at the arm terminator `;;` or at `esac`.
fn download_branch_invokes_failover(script_contents: &str) -> bool {
    const INVOCATION: &str = r#"dep_ch_failover "yt-dlp,ffmpeg""#;
    let mut in_branch = false;
    for line in script_contents.lines() {
        let mut rest = line.trim_start();
        if !in_branch {
            match rest.strip_prefix("download)") {
                Some(after) => {
                    in_branch = true;
                    rest = after.trim_start();
                }
                None => continue,
            }
        }
        if rest.starts_with(INVOCATION) {
            return true;
        }
        let rest = rest.trim_end();
        if rest.ends_with(";;") || rest == "esac" {
            in_branch = false;
        }
    }
    false
}

/// Locate a download-capable tool inside a composed PATH string.
/// `names` comes from [`download_tool_names`]: yt-dlp OR ffmpeg when
/// the active script's `-d` mode accepts either (4.15+), ffmpeg alone
/// for older shapes (aria2c the script checks itself either way).
/// Pure: caller supplies the names, path-list, and the check, so tests
/// drive every branch without touching real disk.
///
/// # Errors
/// [`AniError::FfmpegMissing`] when no accepted tool is found — the
/// frontend modal recommends installing ffmpeg, which remains the
/// primary suggestion either way.
pub fn ensure_download_tool_in_path(
    names: &[&str],
    composed_path: &OsStr,
    is_executable: impl Fn(&Path) -> bool,
) -> Result<()> {
    for dir in std::env::split_paths(composed_path) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        if names.iter().any(|n| is_executable(&dir.join(n))) {
            return Ok(());
        }
    }
    Err(AniError::FfmpegMissing)
}

#[cfg(test)]
#[path = "env_test.rs"]
mod tests;
