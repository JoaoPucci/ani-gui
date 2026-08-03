//! `play_syncplay` — resolve natively, then hand off to Syncplay.
//!
//! Mirror of `commands::play_external_command::play_external`: same
//! cache + fresh-resolve pipeline, terminal action is a Syncplay
//! spawn instead of a direct player spawn. Lives in its own module
//! (not inline in `commands/syncplay.rs`) so the build_argv/
//! open_syncplay tests and the longer cache-reuse pipeline don't
//! share an aggregate-ccn ceiling — the firm CRAP gate gets tripped
//! by file aggregates, not individual functions.

use crate::app::AppState;
use crate::commands::play::PlayArgs;
use crate::commands::play_cache::try_launch_args_from_cache;
use crate::commands::play_external_command::resolve_fresh_for_handoff;
use crate::commands::syncplay::{open_syncplay, SyncplayLaunchArgs};
use crate::config::read_config;
use crate::error::Result;

/// Resolve `args` natively and hand the master playlist URL to the
/// user's locally-installed Syncplay binary. Behaves like
/// `play_external` (same resolution chain, same cache reuse) but the
/// terminal action is a Syncplay spawn. Syncplay handles its own
/// wrapped-player flags internally — the argv we pass is just the
/// URL plus per-stream flags after the `--` separator.
///
/// # Errors
/// Inherits from [`resolve_fresh_for_handoff`] and
/// [`super::syncplay::open_syncplay`] (missing binary, spawn
/// failure).
pub async fn play_syncplay(state: &AppState, args: &PlayArgs) -> Result<()> {
    let cfg = read_config(&state.config_path).unwrap_or_default();

    // Syncplay wraps whichever player the user already configured
    // for "Open in external" — most users have one media player
    // installed, and routing both flows through the same kind keeps
    // the per-stream flag shapes (referer, sub-file) consistent
    // between the two surfaces.
    let player_kind = cfg.external_player_kind;

    // Reuse the long-term cache the same way play_external does — the
    // embedded player likely just resolved this exact (title, mode,
    // quality, episode) tuple. The cached referer + subtitle ride
    // along so Syncplay's wrapped player gets the same flags it
    // would have under play_external.
    if let Some(launch) = try_launch_args_from_cache(state, args, &cfg).await {
        return open_syncplay(&SyncplayLaunchArgs {
            stream_url: launch.stream_url,
            binary: cfg.syncplay_binary,
            referer: launch.referer,
            subtitle_url: launch.subtitle_url,
            player_kind,
            player_binary: cfg.external_player.clone(),
        });
    }

    let resolved = resolve_fresh_for_handoff(state, args).await?;

    open_syncplay(&SyncplayLaunchArgs {
        stream_url: resolved.master_url,
        binary: cfg.syncplay_binary,
        // anidb's streams carry no referer requirement — 5.0's own
        // player invocation dropped the flag with the provider change.
        referer: None,
        subtitle_url: None,
        player_kind,
        player_binary: cfg.external_player,
    })
}

#[cfg(all(test, unix))]
#[path = "play_syncplay_test.rs"]
mod tests;
