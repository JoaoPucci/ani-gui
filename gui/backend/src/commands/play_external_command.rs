//! `play_external` — bridge a Kitsu-resolved title to the user's
//! external media player (default `mpv`).
//!
//! Mirror of `play::play_with_progress`'s resolution chain — same
//! long-term cache reuse, same native walk — but the terminal
//! action is a direct
//! `external_player::open_external_player` spawn instead of a
//! StreamSession + proxy. Lives outside `commands/play.rs` so the
//! play module's lizard ccn stays under the firm CRAP ceiling.

use crate::app::AppState;
use crate::commands::external_player;
use crate::commands::play::PlayArgs;
use crate::commands::play_cache::try_launch_args_from_cache;
use crate::config::read_config;
use crate::error::Result;

/// Resolve `args` through the native walk and hand the stream straight
/// to the user's external player (default `mpv`). No session is
/// registered — the player streams from the upstream directly with
/// the `Referer:` flag.
///
/// # Errors
/// Inherits from the native walk and
/// [`external_player::open_external_player`] (missing binary,
/// non-zero spawn status).
pub async fn play_external(state: &AppState, args: &PlayArgs) -> Result<()> {
    let cfg = read_config(&state.config_path).unwrap_or_default();

    // Long-term cache reuse — same shape as play_with_progress. The
    // embedded player likely just resolved this exact (title, mode,
    // quality, episode) tuple seconds ago; without this the user
    // would wait another 30s for a fresh resolve.
    // HEAD-validate so a stale/dead URL falls through to the fresh
    // path instead of handing mpv a 403.
    if let Some(launch) = try_launch_args_from_cache(state, args, &cfg).await {
        return external_player::open_external_player(&launch);
    }

    let launch = crate::commands::play_handoff::resolve_launch_args(state, args).await?;
    external_player::open_external_player(&launch)
}

#[cfg(all(test, unix))]
#[path = "play_external_command_test.rs"]
mod tests;
