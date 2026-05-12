//! `play_syncplay` — resolve ani-cli, then hand off to Syncplay.
//!
//! Mirror of `commands::play::play_external`: same cache + fresh-
//! resolve pipeline, terminal action is a Syncplay spawn instead of
//! a direct player spawn. Lives in its own module (not inline in
//! `commands/syncplay.rs`) so the build_argv/open_syncplay tests
//! and the longer cache-reuse pipeline don't share an aggregate-ccn
//! ceiling — the firm CRAP gate gets tripped by file aggregates,
//! not individual functions.

use crate::anicli::process::run_debug;
use crate::app::AppState;
use crate::commands::play::{debug_options_for, pick_title_and_index, PlayArgs};
use crate::commands::play_cache::try_launch_args_from_cache;
use crate::commands::play_referer::infer_referer;
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
/// Inherits from [`run_debug`] and
/// [`super::syncplay::open_syncplay`] (missing binary, spawn
/// failure).
pub async fn play_syncplay(state: &AppState, args: &PlayArgs) -> Result<()> {
    let quality = args.quality.as_deref().unwrap_or("best");
    let cfg = read_config(&state.config_path).unwrap_or_default();

    // Reuse the long-term cache the same way play_external does — the
    // embedded player likely just resolved this exact (title, mode,
    // quality, episode) tuple. Without it, the user waits another
    // ~30s for ani-cli to spin up a fresh fetch.
    if let Some(launch) = try_launch_args_from_cache(state, args, &cfg).await {
        // Reuse the cached referer — try_launch_args_from_cache
        // already pulls it from the cache row. fast4speed.rsvp cache
        // rows carry `Referer: https://allmanga.to` so Syncplay's
        // wrapped mpv gets the same header play_external would.
        return open_syncplay(&SyncplayLaunchArgs {
            stream_url: launch.stream_url,
            binary: cfg.syncplay_binary,
            referer: launch.referer,
            // test(red): subtitle threading lands in the paired
            // fix(green) commit; today soft-subtitle streams play
            // under Syncplay but lose subtitles.
            subtitle_url: None,
        });
    }

    let opts = debug_options_for(state, None);
    let (search_title, select_index, _chosen_candidate) = pick_title_and_index(state, args).await;
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

    open_syncplay(&SyncplayLaunchArgs {
        stream_url: resolved.selected_url,
        binary: cfg.syncplay_binary,
        referer,
        // test(red): see comment above.
        subtitle_url: None,
    })
}
