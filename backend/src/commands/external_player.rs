//! `open_external_player` command — escape hatch that launches the
//! user's chosen external media player (default `mpv`) with the
//! referer flag the stream requires, when it requires one.
//!
//! This is never an automatic fallback — it's user-triggered (a button
//! on the in-window player chrome). Auto-fallback would be confusing.

use serde::{Deserialize, Serialize};

use crate::error::{AniError, Result};

#[path = "external_player_template.rs"]
mod template;
use template::{build_argv_custom, build_argv_with_template};

/// Player flavor — controls which flag syntax `build_argv` emits.
///
/// The argv contract differs per player: mpv accepts
/// `--force-media-title=`, VLC accepts `--meta-title=`, IINA forwards
/// mpv flags via `--mpv-` prefixes, and Custom plays it safe by
/// passing only the URL (we don't know what the user's player wants).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalPlayerKind {
    /// mpv — the default, matches the upstream `ani-cli` flag set.
    #[default]
    Mpv,
    /// VideoLAN VLC — different flag names for the same concepts.
    Vlc,
    /// IINA on macOS — wraps mpv, takes flags via `--mpv-` prefix.
    Iina,
    /// Anything else — bare URL only, no flags.
    Custom,
}

/// Arguments to the command, built by the backend alone: the stream
/// from the resolver or the play cache, the player and its settings
/// from the user's config (default `mpv`)
/// ([`crate::commands::play_handoff::launch_args_for`],
/// [`crate::commands::play_cache::cached_launch_args`]). Never decoded
/// from a request, so no caller names the program the backend runs or
/// the arguments it is given.
#[derive(Debug)]
pub struct LaunchArgs {
    /// The resolved stream URL (mp4 or m3u8). Must be an http(s) URL
    /// ([`is_stream_url`]); it goes on the command line bare.
    pub stream_url: String,
    /// Optional `Referer:` value the upstream CDN requires.
    pub referer: Option<String>,
    /// Title shown in the player window's titlebar.
    pub title: Option<String>,
    /// Player command, e.g. `"mpv"`, from the user's settings.
    pub player_command: String,
    /// Which player flag syntax to use, from the user's settings.
    pub player_kind: ExternalPlayerKind,
    /// Free-text args template used only when `player_kind` is
    /// `Custom`. Tokens supported: `{url}`, `{referer}`, `{title}`,
    /// `{subtitle}` (the first sidecar track's URL — the one the
    /// built-in player starts on; none when it starts on none, or for
    /// a show remembered Off).
    /// A token containing a missing/empty placeholder is dropped
    /// from argv entirely (so optional flags don't end up as
    /// `--referrer=` with nothing after the equals).
    pub custom_args_template: Option<String>,
    /// Sidecar subtitle tracks' upstream URLs, most wanted first
    /// ([`crate::commands::play_handoff::subtitle_urls_preferred`]) — a
    /// player that takes one file takes the first, and a command line
    /// that cannot carry them all keeps the first ones
    /// ([`crate::commands::command_line`]). The player fetches them
    /// itself; the referer flag it already gets covers those fetches.
    pub subtitle_urls: Vec<String>,
    /// Whether the show's remembered captions choice is Off: the player
    /// starts with its subtitles off.
    pub subtitles_off: bool,
}

/// Whether `s` can go on a player's command line as the stream: an
/// http or https URL, spelled with its scheme first, so it can never
/// read as an option (`--script=…`, which mpv would run). The value
/// comes from the provider's answer.
#[must_use]
pub fn is_stream_url(s: &str) -> bool {
    (s.starts_with("https://") || s.starts_with("http://"))
        && url::Url::parse(s).is_ok_and(|u| u.host_str().is_some_and(|h| !h.is_empty()))
}

/// Refuse a launch whose stream is not an http(s) URL
/// ([`is_stream_url`]), before anything is spawned.
///
/// # Errors
/// [`AniError::ParseFailed`] when it is not.
pub(crate) fn ensure_stream_url(s: &str) -> Result<()> {
    if is_stream_url(s) {
        Ok(())
    } else {
        Err(AniError::ParseFailed {
            detail: "external launch: the stream is not an http(s) URL".into(),
        })
    }
}

/// Build the argv that would be passed to `Command::new(player).args(...)`.
/// Pure: no spawn happens here so unit tests can lock the contract.
///
/// Order across all kinds: title, referrer, URL last.
#[must_use]
pub fn build_argv(args: &LaunchArgs) -> Vec<String> {
    match args.player_kind {
        ExternalPlayerKind::Mpv => build_argv_with_template(
            args,
            "--force-media-title=",
            "--referrer=",
            SubFlag::Each("--sub-file=", "--sid=no"),
        ),
        // VLC takes one --sub-file: the first track, the one the
        // built-in player starts on — none for a show remembered Off.
        ExternalPlayerKind::Vlc => build_argv_with_template(
            args,
            "--meta-title=",
            "--http-referrer=",
            SubFlag::First("--sub-file="),
        ),
        ExternalPlayerKind::Iina => build_argv_with_template(
            args,
            "--mpv-force-media-title=",
            "--mpv-referrer=",
            SubFlag::Each("--mpv-sub-file=", "--mpv-sid=no"),
        ),
        ExternalPlayerKind::Custom => build_argv_custom(args),
    }
}

/// How a player takes sidecar tracks: one flag per track, or one
/// flag for the first track only.
enum SubFlag {
    /// One flag per track, and the flag that starts the player with
    /// its subtitles off.
    Each(&'static str, &'static str),
    First(&'static str),
}

/// Launch the configured external player with the right argv. Returns
/// once the spawn completes (does not wait for the player to exit).
///
/// On Unix the child inherits a closed stdin/stdout/stderr; the parent
/// never `wait()`s on it, so when ani-gui exits the child is reparented
/// to init (PID 1) and continues independently. That's the behavior
/// the script's `nohup ... &` invocation gives, sufficient for our needs.
/// On Windows, `Command::spawn` without `Wait()` already detaches.
///
/// # Errors
/// - [`AniError::ParseFailed`] if the stream is not an http(s) URL
///   ([`is_stream_url`]); nothing is spawned.
/// - [`AniError::PlayerSpawnFailed`] if the configured player command
///   can't be spawned (usually means it's not on PATH or the path
///   doesn't point at an executable). Carries the configured binary
///   name so the UI can name it in the error toast.
pub fn open_external_player(args: &LaunchArgs) -> Result<()> {
    ensure_stream_url(&args.stream_url)?;
    if args.player_command.trim().is_empty() {
        return Err(AniError::PlayerSpawnFailed {
            binary: args.player_command.clone(),
        });
    }
    let argv = build_argv(args);
    let mut cmd = std::process::Command::new(&args.player_command);
    cmd.args(&argv)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    spawn_detached(&mut cmd).map_err(|_| AniError::PlayerSpawnFailed {
        binary: args.player_command.clone(),
    })
}

/// Spawn with a short retry on ETXTBSY: exec of a freshly written
/// executable can race a concurrent fork that briefly holds its
/// write descriptor (the fds are close-on-exec, so the window is
/// microseconds). Real for just-staged binaries; every other error
/// surfaces immediately.
pub(crate) fn spawn_detached(cmd: &mut std::process::Command) -> std::io::Result<()> {
    for _ in 0..5 {
        match cmd.spawn() {
            Ok(_) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                // Busy: say so to a test holding it, which can then let
                // it go knowing the retry is what it tests.
                #[cfg(test)]
                crate::held::queued(
                    &std::path::Path::new(cmd.get_program())
                        .display()
                        .to_string(),
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => return Err(e),
        }
    }
    cmd.spawn().map(|_| ())
}

#[cfg(test)]
#[path = "external_player_test.rs"]
mod tests;

#[cfg(test)]
#[path = "external_player_subtitles_test.rs"]
mod subtitle_tests;
