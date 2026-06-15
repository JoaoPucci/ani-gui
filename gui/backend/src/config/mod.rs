//! Settings persisted to `$XDG_CONFIG_HOME/ani-gui/config.toml`.
//!
//! User-overridable values: locale, default quality, sub/dub mode,
//! external player command, image cache cap, etc.

pub mod paths;
mod syncplay;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::commands::external_player::ExternalPlayerKind;
use crate::error::{AniError, Result};

use syncplay::default_syncplay_binary;

/// Application configuration. Values default to upstream `ani-cli`'s
/// defaults so a fresh install behaves identically until the user opens
/// Settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "snake_case")]
pub struct Config {
    /// UI locale (BCP 47). Default `"en"`.
    pub locale: String,
    /// `"sub"` or `"dub"`.
    pub mode: String,
    /// Quality token: `"best"`, `"worst"`, `"1080"`, etc.
    pub quality: String,
    /// External-player command for the escape hatch. Default `"mpv"`.
    pub external_player: String,
    /// Which player flag syntax `build_argv` should emit. Default
    /// `Mpv` so existing configs keep their behaviour. The settings
    /// page lets the user switch to `Vlc`, `Iina`, or `Custom`.
    pub external_player_kind: ExternalPlayerKind,
    /// Args template for the `Custom` kind (ignored otherwise).
    /// shlex-split + per-token placeholder substitution at launch
    /// time — see `commands::external_player::build_argv_custom`.
    /// Default empty (triggers the bare-URL fallback).
    pub external_player_custom_args: String,
    /// Path to the user's locally-installed [Syncplay](https://syncplay.pl)
    /// binary, used by the play page's "Watch together" hamburger
    /// entry. Per-OS default: `"syncplay"` on Linux + Windows, the
    /// macOS .app inner executable path on macOS. We don't bundle
    /// Syncplay (heavyweight PyQt5 app; `apt install syncplay` is
    /// broken on Ubuntu 24.04 Noble) — when the spawn fails the
    /// frontend's ErrorOverlay links the user to syncplay.pl.
    #[serde(default = "default_syncplay_binary")]
    pub syncplay_binary: String,
    /// Hard cap on the on-disk image cache, in megabytes.
    pub image_cache_cap_mb: u64,
    /// When `true`, the player auto-advances to the next episode at
    /// `ended`. Opt-in — defaults to `false` so the existing behaviour
    /// (stop at end of episode) is preserved.
    pub auto_play_next: bool,
    /// When `true`, the bottom-of-screen progress strip renders while
    /// any downloads are active. Defaults to `true`. The topbar
    /// download icon + popover dock remain available either way; this
    /// setting only governs the persistent bottom-of-screen surface.
    pub download_bottom_bar_enabled: bool,
    /// When `true`, the player auto-skips opening sequences using
    /// aniskip's crowd-sourced timestamps instead of showing a
    /// manual Skip button. Opt-in — most users want to read /
    /// vibe with the OP at least the first time. Default `false`.
    pub auto_skip_op: bool,
    /// When `true`, the player auto-skips ending sequences. Same
    /// rationale as `auto_skip_op`. Default `false`.
    pub auto_skip_ed: bool,
    /// Toggles between Chromium's native `<video>` controls bar and
    /// our custom controls overlay. Custom gives the timeline the
    /// per-show accent color + lets the fullscreen button target
    /// `.player-frame` so the Skip OP/Outro overlay stays visible
    /// during fullscreen — at the cost of losing the native PiP
    /// menu and caption picker. Default `true` (custom) because the
    /// custom chrome is what the M3 design direction targets and
    /// native is strictly inferior for ani-gui's UI surface.
    pub use_custom_player_controls: bool,
    /// When `true`, navigating away from the player pauses the
    /// video instead of entering Picture-in-Picture. Default
    /// `true` because auto-PiP-on-navigate is a surprising default
    /// (a small floating window follows OS focus when the user
    /// expected Back to halt playback). Users who actively want
    /// PiP can flip this to `false` in Settings.
    pub disable_auto_pip_on_leave: bool,
    /// When `true`, the backend runs `ani-cli -U` against its cached
    /// copy on each boot if the script is older than 24 h. Defaults
    /// to `true` because allmanga's API drifts daily — without
    /// fresh script content, playback breaks for everyone the moment
    /// upstream pushes a hotfix we don't have. Users on metered or
    /// strictly offline setups can flip it off.
    pub auto_update_anicli: bool,
    /// When `true`, the renderer's update notifier considers GitHub
    /// pre-releases when checking for a newer version. Defaults to
    /// `true` because every ani-gui release shipped so far is marked
    /// prerelease — turning it off makes the notifier silent until
    /// the first stable cut. Users who want only stable releases can
    /// flip it off in Settings.
    pub update_include_prereleases: bool,
    /// Which connected tracker is the "primary" one — drives the
    /// topbar chip/avatar and the Watch Later rail's lead provider
    /// when more than one account is connected. Empty string (the
    /// default) means "no explicit choice"; the UI then falls back to
    /// its fixed AniList-first precedence. Stored as the provider slug
    /// (`"anilist"` / `"mal"`) rather than an enum so an unknown value
    /// written by a newer build degrades to the fallback instead of
    /// failing to deserialize. Note this only affects what's
    /// displayed/led — progress write-back still fans out to every
    /// connected provider regardless.
    #[serde(default)]
    pub primary_account: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            locale: "en".into(),
            mode: "sub".into(),
            quality: "best".into(),
            external_player: "mpv".into(),
            external_player_kind: ExternalPlayerKind::Mpv,
            external_player_custom_args: String::new(),
            syncplay_binary: default_syncplay_binary(),
            image_cache_cap_mb: 500,
            auto_play_next: false,
            download_bottom_bar_enabled: true,
            auto_skip_op: false,
            auto_skip_ed: false,
            use_custom_player_controls: true,
            disable_auto_pip_on_leave: true,
            auto_update_anicli: true,
            update_include_prereleases: true,
            primary_account: String::new(),
        }
    }
}

/// Read the config file at `path`. Missing-file returns
/// `Config::default()` so a fresh install behaves like an unconfigured
/// `ani-cli` user.
///
/// # Errors
/// - [`AniError::Io`] if the file exists but cannot be read.
/// - [`AniError::Config`] if the file isn't valid TOML or has
///   incompatible types.
pub fn read_config(path: &Path) -> Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let body = std::fs::read_to_string(path)?;
    let cfg: Config = toml::from_str(&body)?;
    Ok(cfg)
}

/// Atomically write `cfg` to `path` (writes to `path.new` then renames).
/// Creates the parent directory if absent.
///
/// # Errors
/// - [`AniError::Io`] on filesystem failures.
/// - [`AniError::Config`] if TOML serialization fails (shouldn't in practice).
pub fn write_config(path: &Path, cfg: &Config) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| AniError::Io)?;
    }
    let body = toml::to_string_pretty(cfg).map_err(|_| AniError::Config)?;
    let tmp = path.with_extension("toml.new");
    std::fs::write(&tmp, body).map_err(|_| AniError::Io)?;
    std::fs::rename(&tmp, path).map_err(|_| AniError::Io)?;
    Ok(())
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
