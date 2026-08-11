//! `play_syncplay` — resolve natively, then hand off to Syncplay.
//!
//! Mirror of `commands::play::play_external`: same cache + fresh-
//! resolve pipeline, terminal action is a Syncplay spawn instead of
//! a direct player spawn. Lives in its own module (not inline in
//! `commands/syncplay.rs`) so the build_argv/open_syncplay tests
//! and the longer cache-reuse pipeline don't share an aggregate-ccn
//! ceiling — the firm CRAP gate gets tripped by file aggregates,
//! not individual functions.

use crate::app::AppState;
use crate::commands::play::PlayArgs;
use crate::commands::play_cache::try_launch_args_from_cache;
use crate::commands::syncplay::{open_syncplay, SyncplayLaunchArgs};
use crate::config::read_config;
use crate::error::Result;

/// Resolve `args` against ani-cli and hand the upstream URL to the
/// user's locally-installed Syncplay binary. Behaves like
/// `play::play_external` (same resolution chain, same cache reuse,
/// same referer-inference) but the terminal action is a Syncplay
/// spawn instead of a direct player spawn. Syncplay handles its own
/// wrapped-player flags internally — the argv we pass is just the
/// URL plus an optional `--referrer=` after the `--` separator.
///
/// # Errors
/// Inherits from the native walk and
/// [`super::syncplay::open_syncplay`] (missing binary, spawn
/// failure).
pub async fn play_syncplay(state: &AppState, args: &PlayArgs) -> Result<()> {
    let cfg = read_config(&state.config_path).unwrap_or_default();

    // Reuse the long-term cache the same way play_external does — the
    // embedded player likely just resolved this exact (title, mode,
    // quality, episode) tuple. Without it, the user waits another
    // ~30s for ani-cli to spin up a fresh fetch.
    // Syncplay wraps whichever player the user already configured
    // for "Open in external" — most users have one media player
    // installed, and routing both flows through the same kind keeps
    // the per-stream flag shapes (referer) consistent between the
    // two surfaces.
    let player_kind = cfg.external_player_kind;

    // Reuse the long-term cache the same way play_external does — the
    // embedded player likely just resolved this exact (title, mode,
    // quality, episode) tuple. The cached referer rides along so
    // Syncplay's wrapped player gets the same flags it would have
    // under play_external.
    if let Some(launch) = try_launch_args_from_cache(state, args, &cfg).await {
        return open_syncplay(&SyncplayLaunchArgs {
            stream_url: launch.stream_url,
            binary: cfg.syncplay_binary,
            referer: launch.referer,
            player_kind,
            player_binary: cfg.external_player.clone(),
        });
    }

    let launch = crate::commands::play_handoff::resolve_launch_args(state, args).await?;

    open_syncplay(&SyncplayLaunchArgs {
        stream_url: launch.stream_url,
        binary: cfg.syncplay_binary,
        referer: launch.referer,
        player_kind,
        player_binary: cfg.external_player,
    })
}

#[cfg(all(test, unix))]
#[path = "play_syncplay_test.rs"]
mod tests;
