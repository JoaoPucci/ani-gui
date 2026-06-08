//! `open_external_player` command — escape hatch that launches the
//! user's chosen external media player (default `mpv`) with the same
//! `--referer` and `--sub-file` flags `ani-cli` passes today.
//!
//! This is never an automatic fallback — it's user-triggered (a button
//! on the in-window player chrome). Auto-fallback would be confusing.

use serde::{Deserialize, Serialize};

use crate::error::{AniError, Result};

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
/// optional referer + optional subtitle. The player command itself comes
/// from the user's config (default `mpv`).
#[derive(Debug, Deserialize)]
pub struct LaunchArgs {
    /// The resolved stream URL (mp4 or m3u8).
    pub stream_url: String,
    /// Optional `Referer:` value the upstream CDN requires.
    pub referer: Option<String>,
    /// Optional subtitle URL (`.vtt`).
    pub subtitle_url: Option<String>,
    /// Title shown in the player window's titlebar.
    pub title: Option<String>,
    /// Player command, e.g. `"mpv"`. Caller resolves this from settings.
    pub player_command: String,
    /// Which player flag syntax to use. Old payloads without this
    /// field decode as `Mpv` so existing clients keep working.
    #[serde(default)]
    pub player_kind: ExternalPlayerKind,
    /// Free-text args template used only when `player_kind` is
    /// `Custom`. Tokens supported: `{url}`, `{referer}`, `{title}`,
    /// `{sub}`. A token containing a missing/empty placeholder is
    /// dropped from argv entirely (so optional flags don't end up
    /// as `--sub-file=` with nothing after the equals).
    #[serde(default)]
    pub custom_args_template: Option<String>,
}

/// Build the argv that would be passed to `Command::new(player).args(...)`.
/// Pure: no spawn happens here so unit tests can lock the contract.
///
/// Order across all kinds: title, sub, referrer, URL last. Matches
/// what `ani-cli`'s `play_episode` mpv branch constructs (lines
/// 394-402 of the script).
#[must_use]
pub fn build_argv(args: &LaunchArgs) -> Vec<String> {
    match args.player_kind {
        ExternalPlayerKind::Mpv => {
            build_argv_with_template(args, "--force-media-title=", "--sub-file=", "--referrer=")
        }
        ExternalPlayerKind::Vlc => {
            build_argv_with_template(args, "--meta-title=", "--sub-file=", "--http-referrer=")
        }
        ExternalPlayerKind::Iina => build_argv_with_template(
            args,
            "--mpv-force-media-title=",
            "--sub-file=",
            "--mpv-referrer=",
        ),
        ExternalPlayerKind::Custom => build_argv_custom(args),
    }
}

/// Shared argv assembly for the three known players — same shape,
/// different flag names.
fn build_argv_with_template(
    args: &LaunchArgs,
    title_flag: &str,
    sub_flag: &str,
    referrer_flag: &str,
) -> Vec<String> {
    let mut argv = Vec::with_capacity(4);
    if let Some(t) = &args.title {
        argv.push(format!("{title_flag}{t}"));
    }
    if let Some(s) = &args.subtitle_url {
        argv.push(format!("{sub_flag}{s}"));
    }
    if let Some(r) = &args.referer {
        argv.push(format!("{referrer_flag}{r}"));
    }
    argv.push(args.stream_url.clone());
    argv
}

/// Build argv for the Custom kind by shlex-splitting the template
/// and substituting placeholders per token. A token containing a
/// missing/empty placeholder is dropped from argv entirely so the
/// user can write `--sub={sub}` without it landing as `--sub=` when
/// no subtitle is available.
///
/// Empty/None template falls back to URL only.
fn build_argv_custom(args: &LaunchArgs) -> Vec<String> {
    let template = match args.custom_args_template.as_deref() {
        Some(s) if !s.trim().is_empty() => s,
        _ => return vec![args.stream_url.clone()],
    };
    let tokens = match shlex::split(template) {
        Some(t) => t,
        // Bad quoting in the template — fall back to bare URL so the
        // user at least sees the stream open instead of silently
        // failing.
        None => return vec![args.stream_url.clone()],
    };
    let referer = args.referer.as_deref().unwrap_or("");
    let title = args.title.as_deref().unwrap_or("");
    let sub = args.subtitle_url.as_deref().unwrap_or("");
    let url = args.stream_url.as_str();
    tokens
        .into_iter()
        .filter_map(|tok| substitute_token(&tok, url, referer, title, sub))
        .collect()
}

/// Returns `Some(rendered)` when every placeholder in `tok` had a
/// non-empty value, `None` if any placeholder was empty (drop rule).
/// `{url}` is always present — tokens containing only `{url}` always
/// render. Unknown `{...}` placeholders pass through verbatim.
fn substitute_token(tok: &str, url: &str, referer: &str, title: &str, sub: &str) -> Option<String> {
    let mut out = String::with_capacity(tok.len());
    let mut chars = tok.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '{' {
            out.push(c);
            continue;
        }
        // Read placeholder name up to `}`.
        let mut name = String::new();
        let mut closed = false;
        for nc in chars.by_ref() {
            if nc == '}' {
                closed = true;
                break;
            }
            name.push(nc);
        }
        if !closed {
            // Unterminated `{...` — pass through literally.
            out.push('{');
            out.push_str(&name);
            continue;
        }
        let value = match name.as_str() {
            "url" => url,
            "referer" => referer,
            "title" => title,
            "sub" => sub,
            // Unknown placeholder — preserve verbatim.
            other => {
                out.push('{');
                out.push_str(other);
                out.push('}');
                continue;
            }
        };
        if value.is_empty() {
            // Drop the entire token: a flag with an empty value is
            // worse than no flag at all.
            return None;
        }
        out.push_str(value);
    }
    Some(out)
}

/// Launch the configured external player with the right argv. Returns
/// once the spawn completes (does not wait for the player to exit).
///
/// On Unix the child inherits a closed stdin/stdout/stderr; the parent
/// never `wait()`s on it, so when ani-gui exits the child is reparented
/// to init (PID 1) and continues independently. That's the behavior
/// `ani-cli`'s `nohup ... &` invocation gives, sufficient for our needs.
/// On Windows, `Command::spawn` without `Wait()` already detaches.
///
/// # Errors
/// - [`AniError::PlayerSpawnFailed`] if the configured player command
///   can't be spawned (usually means it's not on PATH or the path
///   doesn't point at an executable). Carries the configured binary
///   name so the UI can name it in the error toast.
pub fn open_external_player(args: &LaunchArgs) -> Result<()> {
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
    cmd.spawn()
        .map(|_| ())
        .map_err(|_| AniError::PlayerSpawnFailed {
            binary: args.player_command.clone(),
        })
}

#[cfg(test)]
#[path = "external_player_test.rs"]
mod tests;
