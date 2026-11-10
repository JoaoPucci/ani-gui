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

/// Arguments to the command. Frontend supplies the resolved stream URL +
/// optional referer. The player command itself comes from the user's
/// config (default `mpv`).
#[derive(Debug, Clone, Deserialize)]
pub struct LaunchArgs {
    /// The resolved stream URL (mp4 or m3u8).
    pub stream_url: String,
    /// Optional `Referer:` value the upstream CDN requires.
    pub referer: Option<String>,
    /// Title shown in the player window's titlebar.
    pub title: Option<String>,
    /// Player command, e.g. `"mpv"`. Caller resolves this from settings.
    /// Blank runs the type's standard program ([`player_program`]).
    pub player_command: String,
    /// Which player flag syntax to use. Old payloads without this
    /// field decode as `Mpv` so existing clients keep working.
    #[serde(default)]
    pub player_kind: ExternalPlayerKind,
    /// Free-text args template used only when `player_kind` is
    /// `Custom`. Tokens supported: `{url}`, `{referer}`, `{title}`,
    /// `{subtitle}` (the first sidecar track's URL — the one the
    /// built-in player starts on; none when it starts on none, or for
    /// a show remembered Off).
    /// A token containing a missing/empty placeholder is dropped
    /// from argv entirely (so optional flags don't end up as
    /// `--referrer=` with nothing after the equals).
    #[serde(default)]
    pub custom_args_template: Option<String>,
    /// Sidecar subtitle tracks' upstream URLs, most wanted first
    /// ([`crate::commands::play_handoff::subtitle_urls_preferred`]) — a
    /// player that takes one file takes the first, and a command line
    /// that cannot carry them all keeps the first ones
    /// ([`crate::commands::command_line`]). The player fetches them
    /// itself; the referer flag it already gets covers those fetches.
    #[serde(default)]
    pub subtitle_urls: Vec<String>,
    /// Whether the show's remembered captions choice is Off: the player
    /// starts with its subtitles off.
    #[serde(default)]
    pub subtitles_off: bool,
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
/// - [`AniError::PlayerSpawnFailed`] if the program can't be spawned
///   (it was not found where it is installed, or the path doesn't
///   point at an executable). Carries the program that was
///   tried: the full path when the standard program was found
///   installed, its bare name when it was not, the command as written
///   otherwise. The UI names it in the error toast. A blank Custom command carries the
///   blank: there is no program to name.
pub fn open_external_player(args: &LaunchArgs) -> Result<()> {
    let Some(program) = launch_program_with(
        &args.player_command,
        args.player_kind,
        &crate::commands::player_locate::locate_installed,
    ) else {
        return Err(AniError::PlayerSpawnFailed {
            binary: args.player_command.clone(),
        });
    };
    // The argv is fitted to the program that launches.
    let launching = LaunchArgs {
        player_command: program.clone(),
        ..args.clone()
    };
    let argv = build_argv(&launching);
    let mut cmd = std::process::Command::new(&program);
    cmd.args(&argv)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    spawn_detached(&mut cmd).map_err(|_| AniError::PlayerSpawnFailed { binary: program })
}

/// The program a launch runs: the command the user configured, or,
/// when it is blank, the type's standard program. A blank Custom
/// command runs nothing: Custom has no standard program.
#[must_use]
pub fn player_program(command: &str, kind: ExternalPlayerKind) -> Option<&str> {
    if command.trim().is_empty() {
        kind.standard_program()
    } else {
        Some(command)
    }
}

/// [`open_external_player`] off the async workers: the lookup it runs
/// may start reg.exe on Windows, and the spawn itself blocks briefly.
///
/// # Errors
/// As [`open_external_player`]. A launch task that dies reads as a
/// spawn failure for the configured command.
pub async fn open_external_player_off_workers(args: LaunchArgs) -> Result<()> {
    let failure = launch_task_failure(&args);
    tokio::task::spawn_blocking(move || open_external_player(&args))
        .await
        .unwrap_or(Err(failure))
}

/// What a launch reports when its task dies before answering.
#[must_use]
pub fn launch_task_failure(args: &LaunchArgs) -> AniError {
    AniError::PlayerSpawnFailed {
        binary: player_program(&args.player_command, args.player_kind)
            .unwrap_or_default()
            .to_string(),
    }
}

/// Settings' answer when the lookup task dies before answering.
#[must_use]
pub fn standard_program_unknown(kind: ExternalPlayerKind) -> StandardProgram {
    standard_program_found_on(crate::commands::player_locate::current_os(), kind, &|_| {
        None
    })
}

/// The program a launch spawns: [`player_program`], with a standard
/// program's bare name replaced by where `locate` finds it installed.
/// A name `locate` cannot find is spawned as it is.
#[must_use]
pub fn launch_program_with(
    command: &str,
    kind: ExternalPlayerKind,
    locate: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let program = player_program(command, kind)?;
    Some(installed_or_as_is(program, locate))
}

/// `program` as `locate` finds it installed, when it is a standard
/// program's bare name. Otherwise, or when nothing is found, as it is.
fn installed_or_as_is(program: &str, locate: &dyn Fn(&str) -> Option<String>) -> String {
    crate::commands::player_locate::standard_name(program)
        .and_then(locate)
        .unwrap_or_else(|| program.to_string())
}

/// The player Syncplay is told to wrap (`--player-path`), from the
/// configured command.
#[must_use]
pub fn syncplay_player_with(
    command: &str,
    _kind: ExternalPlayerKind,
    locate: &dyn Fn(&str) -> Option<String>,
) -> String {
    // Blank: no --player-path, so Syncplay opens the player configured
    // in Syncplay itself, as it always did.
    if command.trim().is_empty() {
        return String::new();
    }
    installed_or_as_is(command, locate)
}

/// What a blank command runs for `kind` on this computer: the standard
/// program's name, and where `locate` finds it installed. Settings
/// shows it under the External player field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StandardProgram {
    /// The program's name (`"vlc"`); none for Custom.
    pub name: Option<&'static str>,
    /// Where it is installed; none when it was not found.
    pub path: Option<String>,
    /// Whether the program exists for this OS at all (IINA is
    /// macOS-only).
    pub supported: bool,
}

/// [`StandardProgram`] for `kind`, found by `locate`.
#[must_use]
pub fn standard_program_found(
    kind: ExternalPlayerKind,
    locate: &dyn Fn(&str) -> Option<String>,
) -> StandardProgram {
    standard_program_found_on(crate::commands::player_locate::current_os(), kind, locate)
}

/// [`standard_program_found`] on `os`.
#[must_use]
pub fn standard_program_found_on(
    os: crate::commands::player_locate::Os,
    kind: ExternalPlayerKind,
    locate: &dyn Fn(&str) -> Option<String>,
) -> StandardProgram {
    let name = kind.standard_program();
    // IINA is a macOS app: there is nothing to find anywhere else.
    let supported =
        !(kind == ExternalPlayerKind::Iina && os != crate::commands::player_locate::Os::MacOs);
    StandardProgram {
        name,
        path: if supported {
            name.and_then(locate)
        } else {
            None
        },
        supported,
    }
}

impl ExternalPlayerKind {
    /// The program this type runs when the user wrote no command: its
    /// own name, looked for where it is installed
    /// ([`crate::commands::player_locate`]). None for Custom.
    #[must_use]
    pub fn standard_program(self) -> Option<&'static str> {
        match self {
            Self::Mpv => Some("mpv"),
            Self::Vlc => Some("vlc"),
            Self::Iina => Some("iina"),
            Self::Custom => None,
        }
    }
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

#[cfg(test)]
#[path = "external_player_prop_test.rs"]
mod prop_tests;
