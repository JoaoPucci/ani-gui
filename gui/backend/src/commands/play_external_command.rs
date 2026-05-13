//! `play_external` — bridge a Kitsu-resolved title to the user's
//! external media player (default `mpv`).
//!
//! Mirror of `play::play_with_progress`'s resolution chain — same
//! long-term cache reuse, same allmanga disambiguation via
//! `pick_title_and_index` — but the terminal action is a direct
//! `external_player::open_external_player` spawn instead of a
//! StreamSession + proxy. Lives outside `commands/play.rs` so the
//! play module's lizard ccn stays under the firm CRAP ceiling.

use crate::anicli::process::run_debug;
use crate::app::AppState;
use crate::commands::external_player::{self, LaunchArgs};
use crate::commands::play::{debug_options_for, pick_title_and_index, PlayArgs};
use crate::commands::play_cache::try_launch_args_from_cache;
use crate::commands::play_referer::infer_referer;
use crate::config::read_config;
use crate::error::{AniError, Result};

/// Resolve `args` against ani-cli and hand the upstream URL straight
/// to the user's external player (default `mpv`). No session is
/// registered — the player streams from the upstream directly with
/// the `Referer:` flag.
///
/// # Errors
/// Inherits from [`run_debug`] and
/// [`external_player::open_external_player`] (missing binary,
/// non-zero spawn status).
pub async fn play_external(state: &AppState, args: &PlayArgs) -> Result<()> {
    let quality = args.quality.as_deref().unwrap_or("best");
    let cfg = read_config(&state.config_path).unwrap_or_default();

    // Long-term cache reuse — same shape as play_with_progress. The
    // embedded player likely just resolved this exact (title, mode,
    // quality, episode) tuple seconds ago; without this the user
    // would wait another 30s for ani-cli to spin up a fresh fetch.
    // HEAD-validate so a stale/dead URL falls through to the fresh
    // path instead of handing mpv a 403.
    if let Some(launch) = try_launch_args_from_cache(state, args, &cfg).await {
        return external_player::open_external_player(&launch);
    }

    // play_external is always a click — never a prefetch — so no
    // hist_dir override needed.
    let opts = debug_options_for(state, None);
    let picked = pick_title_and_index(state, args).await;
    let search_title = picked.title;
    let select_index = picked.index;
    let chosen_candidate = picked.candidate;
    if chosen_candidate.is_none() {
        return Err(if picked.any_search_succeeded {
            AniError::NoResults
        } else {
            AniError::Network
        });
    }
    let resolved = run_debug(
        &opts,
        &search_title,
        &args.episode,
        quality,
        &args.mode,
        select_index,
    )
    .await?;

    let referer = infer_referer(&resolved);

    let launch = LaunchArgs {
        stream_url: resolved.selected_url,
        referer,
        subtitle_url: resolved.subtitle_url,
        title: Some(format!("{} · ep {}", args.title, args.episode)),
        player_command: cfg.external_player,
        player_kind: cfg.external_player_kind,
        custom_args_template: Some(cfg.external_player_custom_args),
    };
    external_player::open_external_player(&launch)
}
