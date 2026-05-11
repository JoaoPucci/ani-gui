//! `open_syncplay` command — launches the user's locally-installed
//! [Syncplay](https://syncplay.pl) binary on a resolved stream URL.
//!
//! Syncplay is a third-party PyQt5 application that wraps the user's
//! own mpv (or vlc/iina) and connects to a Syncplay server to keep
//! room members' playback in sync. ani-gui hands it the resolved
//! upstream URL as the positional file argument; Syncplay handles
//! everything else (room dialog, server connection, wrapped-player
//! flags) in its own UI.
//!
//! Unlike the external-player escape hatch, the user does NOT pick a
//! "kind" — Syncplay's argv shape is uniform across platforms. We
//! also don't forward `--referer` / `--sub-file` / `--title`:
//! Syncplay's own command line for the wrapped player varies by
//! player + version, so users who need those should configure them
//! in their player's own config (`~/.config/mpv/mpv.conf`, etc.).
//!
//! Bundling is intentionally out of scope — Syncplay is a heavyweight
//! PyQt5 app and `apt install syncplay` is broken on Ubuntu 24.04
//! (Noble ships 1.7.0 which crashes on Python 3.12). When the
//! configured binary can't be spawned, the frontend surfaces an
//! `ErrorOverlay` modal with a link to syncplay.pl — same pattern as
//! the ffmpeg-missing dialog.

use serde::Deserialize;

use crate::error::{AniError, Result};

/// Arguments to the command. Caller supplies the resolved stream URL
/// (the play flow's same URL the embedded player would consume) and
/// the configured Syncplay binary path. Frontend reads the binary
/// from `Config::syncplay_binary`.
#[derive(Debug, Deserialize)]
pub struct SyncplayLaunchArgs {
    /// The resolved stream URL (mp4 or m3u8). Syncplay's positional
    /// "file" argument.
    pub stream_url: String,
    /// Syncplay binary path. Resolved from `Config::syncplay_binary`
    /// (per-OS default; user-overridable in settings).
    pub binary: String,
}

/// Build the argv that would be passed to `Command::new(binary).args(...)`.
/// Pure: no spawn happens here so unit tests can lock the contract.
#[must_use]
pub fn build_argv(args: &SyncplayLaunchArgs) -> Vec<String> {
    vec![args.stream_url.clone()]
}

/// Launch the configured Syncplay binary against the resolved stream
/// URL. Returns once the spawn completes; the child is detached the
/// same way external_player.rs detaches the user's mpv.
///
/// # Errors
/// - [`AniError::SyncplaySpawnFailed`] if the configured binary can't
///   be spawned (not on PATH, doesn't exist, or path doesn't point at
///   an executable). Carries the binary string so the UI can name
///   the failed command in the error dialog and link the user to
///   <https://syncplay.pl/download/>.
pub fn open_syncplay(args: &SyncplayLaunchArgs) -> Result<()> {
    if args.binary.trim().is_empty() {
        return Err(AniError::SyncplaySpawnFailed {
            binary: args.binary.clone(),
        });
    }
    let argv = build_argv(args);
    let mut cmd = std::process::Command::new(&args.binary);
    cmd.args(&argv)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    cmd.spawn()
        .map(|_| ())
        .map_err(|_| AniError::SyncplaySpawnFailed {
            binary: args.binary.clone(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(stream: &str, binary: &str) -> SyncplayLaunchArgs {
        SyncplayLaunchArgs {
            stream_url: stream.into(),
            binary: binary.into(),
        }
    }

    #[test]
    fn argv_is_just_the_url() {
        // Syncplay's command line accepts the file (URL) as a single
        // positional argument. We don't pass referer / title / sub
        // because Syncplay's wrapped-player flag shape varies by
        // player + version — users who need that should configure
        // their player's own defaults.
        let v = build_argv(&args("https://example.com/master.m3u8", "syncplay"));
        assert_eq!(v, vec!["https://example.com/master.m3u8".to_string()]);
    }

    #[test]
    fn open_syncplay_with_blank_binary_returns_spawn_failed() {
        // Blank binary is a misconfigured-settings case; treat it the
        // same as "binary not found" so the frontend can surface the
        // same syncplay.pl install pointer.
        let r = open_syncplay(&args("https://example.com/v.mp4", ""));
        match r {
            Err(AniError::SyncplaySpawnFailed { binary }) => assert!(binary.is_empty()),
            other => panic!("expected SyncplaySpawnFailed, got {other:?}"),
        }
    }

    #[test]
    fn open_syncplay_with_unknown_binary_carries_binary_name() {
        // The whole point of the typed variant: the frontend can name
        // which binary failed in the error dialog. Pin that the
        // configured value flows through verbatim.
        let r = open_syncplay(&args(
            "https://example.com/v.mp4",
            "__definitely_not_a_real_syncplay__",
        ));
        match r {
            Err(AniError::SyncplaySpawnFailed { binary }) => {
                assert_eq!(binary, "__definitely_not_a_real_syncplay__");
            }
            other => panic!("expected SyncplaySpawnFailed, got {other:?}"),
        }
    }
}
