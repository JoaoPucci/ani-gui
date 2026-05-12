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
/// (the play flow's same URL the embedded player would consume), the
/// configured Syncplay binary path, and an optional `Referer:` value
/// that gets forwarded to Syncplay's wrapped mpv. Frontend reads the
/// binary from `Config::syncplay_binary`; the referer is inferred by
/// `play_syncplay` from the resolved upstream (mirrors the
/// `play_external` path's fast4speed.rsvp → allmanga.to fallback).
#[derive(Debug, Deserialize)]
pub struct SyncplayLaunchArgs {
    /// The resolved stream URL (mp4 or m3u8). Syncplay's positional
    /// "file" argument.
    pub stream_url: String,
    /// Syncplay binary path. Resolved from `Config::syncplay_binary`
    /// (per-OS default; user-overridable in settings).
    pub binary: String,
    /// Optional `Referer:` header value the upstream CDN requires.
    /// Forwarded to Syncplay's wrapped player via the mpv-style
    /// `--referrer=` flag after the `--` separator. fast4speed.rsvp
    /// 403s without `Referer: https://allmanga.to`, so the same
    /// inference logic `play_external` uses applies to Syncplay too.
    /// Old payloads without this field decode as `None`.
    #[serde(default)]
    pub referer: Option<String>,
    /// Optional sidecar subtitle URL (`.vtt`) when ani-cli surfaces a
    /// soft-subtitle track separately from the stream. Forwarded to
    /// the wrapped mpv via `--sub-file=`. Without this, Syncplay's
    /// wrapped player opens the video but drops the subtitles even
    /// though the embedded and external-player paths show them.
    /// Old payloads without this field decode as `None`.
    #[serde(default)]
    pub subtitle_url: Option<String>,
}

/// Build the argv that would be passed to `Command::new(binary).args(...)`.
/// Pure: no spawn happens here so unit tests can lock the contract.
///
/// Syncplay's CLI grammar is `syncplay [options] [file] -- [player
/// options]`. The `--` separator forwards everything after it to the
/// wrapped player (mpv by default). When `referer` is set, we emit
/// the mpv-style `--referrer=` flag past the `--`; this is the
/// minimum needed so fast4speed.rsvp's referer-required CDNs don't
/// 403 out from under Syncplay's mpv. Title / sub-file forwarding
/// stays out of scope (see `.planning/follow-ups.md`).
#[must_use]
pub fn build_argv(args: &SyncplayLaunchArgs) -> Vec<String> {
    let mut argv = vec![args.stream_url.clone()];
    if let Some(referer) = args.referer.as_deref() {
        if !referer.is_empty() {
            argv.push("--".to_string());
            argv.push(format!("--referrer={referer}"));
        }
    }
    argv
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
            referer: None,
            subtitle_url: None,
        }
    }

    #[test]
    fn argv_with_no_referer_is_just_the_url() {
        // Bare URL is the no-referer baseline. Most catalogues work
        // this way; only referer-required CDNs (fast4speed.rsvp)
        // exercise the forwarding path.
        let v = build_argv(&args("https://example.com/master.m3u8", "syncplay"));
        assert_eq!(v, vec!["https://example.com/master.m3u8".to_string()]);
    }

    #[test]
    fn argv_forwards_referer_after_separator() {
        // Syncplay's CLI grammar is `syncplay [options] [file] --
        // [player options]`. The `--` separator hands the rest to
        // the wrapped player (mpv by default), so the mpv-style
        // `--referrer=` flag is what reaches the upstream CDN.
        // Without this, fast4speed.rsvp 403s under Syncplay's mpv
        // even though play_external can play the same URL.
        let mut a = args("https://example.com/master.m3u8", "syncplay");
        a.referer = Some("https://allmanga.to".into());
        let v = build_argv(&a);
        assert_eq!(
            v,
            vec![
                "https://example.com/master.m3u8".to_string(),
                "--".to_string(),
                "--referrer=https://allmanga.to".to_string(),
            ]
        );
    }

    #[test]
    fn argv_drops_empty_referer() {
        // An empty-string referer is no better than no referer at
        // all — emitting `--referrer=` with nothing after it would
        // make mpv complain. Drop the whole `--` block in that case.
        let mut a = args("https://example.com/v.mp4", "syncplay");
        a.referer = Some(String::new());
        let v = build_argv(&a);
        assert_eq!(v, vec!["https://example.com/v.mp4".to_string()]);
    }

    #[test]
    fn launch_args_decode_without_referer_for_back_compat() {
        // Old client payloads (pre-referer-forwarding) don't include
        // the `referer` field. They must still decode and default to
        // None.
        let json = r#"{
            "stream_url": "https://example.com/v.mp4",
            "binary": "syncplay"
        }"#;
        let a: SyncplayLaunchArgs =
            serde_json::from_str(json).expect("decodes with default referer");
        assert!(a.referer.is_none());
        assert!(a.subtitle_url.is_none());
    }

    #[test]
    fn argv_forwards_subtitle_after_separator() {
        // Soft-subtitle streams: ani-cli's parser surfaces a sidecar
        // `.vtt` URL alongside the stream. play_external forwards it
        // as `--sub-file=`; Syncplay's wrapped mpv needs the same
        // flag past the `--` separator, or the user sees the video
        // play but loses subtitles.
        let mut a = args("https://example.com/master.m3u8", "syncplay");
        a.subtitle_url = Some("https://example.com/subs.vtt".into());
        let v = build_argv(&a);
        assert_eq!(
            v,
            vec![
                "https://example.com/master.m3u8".to_string(),
                "--".to_string(),
                "--sub-file=https://example.com/subs.vtt".to_string(),
            ]
        );
    }

    #[test]
    fn argv_forwards_referer_and_subtitle_together() {
        // Both flags share one `--` separator. Order matches mpv's
        // typical argv shape: title-y flags first (none here), then
        // sub-file, then referrer, then the URL — but here the URL
        // is the file argument BEFORE `--`, so post-separator order
        // is what counts: referrer then sub-file (stable across CDN
        // shapes that supply both).
        let mut a = args("https://example.com/master.m3u8", "syncplay");
        a.referer = Some("https://allmanga.to".into());
        a.subtitle_url = Some("https://example.com/subs.vtt".into());
        let v = build_argv(&a);
        assert_eq!(
            v,
            vec![
                "https://example.com/master.m3u8".to_string(),
                "--".to_string(),
                "--referrer=https://allmanga.to".to_string(),
                "--sub-file=https://example.com/subs.vtt".to_string(),
            ]
        );
    }

    #[test]
    fn argv_drops_empty_subtitle() {
        // Defensive: an empty `subtitle_url` falls through the same
        // way an empty `referer` does — emitting `--sub-file=` with
        // nothing after the equals just makes mpv complain.
        let mut a = args("https://example.com/v.mp4", "syncplay");
        a.subtitle_url = Some(String::new());
        let v = build_argv(&a);
        assert_eq!(v, vec!["https://example.com/v.mp4".to_string()]);
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
