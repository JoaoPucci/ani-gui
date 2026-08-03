//! `play_external` — bridge a Kitsu-resolved title to the user's
//! external media player (default `mpv`).
//!
//! Mirror of `play::play_with_progress`'s resolution chain — same
//! long-term cache reuse, same native anidb walk — but the terminal
//! action is a direct `external_player::open_external_player` spawn
//! instead of a StreamSession + proxy. Lives outside
//! `commands/play.rs` so the play module's lizard ccn stays under the
//! firm CRAP ceiling.

use crate::app::AppState;
use crate::commands::external_player::{self, LaunchArgs};
use crate::commands::play::{anidb_client, native_gate_outcome, PlayArgs};
use crate::commands::play_cache::try_launch_args_from_cache;
use crate::commands::play_native_resolve::{resolve_native, NativeResolveRequest, NativeResolved};
use crate::config::read_config;
use crate::error::Result;

/// Resolve `args` natively and hand the master playlist URL straight
/// to the user's external player (default `mpv`). No session is
/// registered — the player streams from the upstream directly.
///
/// # Errors
/// Inherits from [`resolve_fresh_for_handoff`] and
/// [`external_player::open_external_player`] (missing binary,
/// non-zero spawn status).
pub async fn play_external(state: &AppState, args: &PlayArgs) -> Result<()> {
    let cfg = read_config(&state.config_path).unwrap_or_default();

    // Long-term cache reuse — same shape as play_with_progress. The
    // embedded player likely just resolved this exact (title, mode,
    // quality, episode) tuple seconds ago; without this the user
    // would wait for a fresh provider walk. HEAD-validate so a
    // stale/dead URL falls through to the fresh path instead of
    // handing mpv a 403.
    if let Some(launch) = try_launch_args_from_cache(state, args, &cfg).await {
        return external_player::open_external_player(&launch);
    }

    let resolved = resolve_fresh_for_handoff(state, args).await?;

    let launch = LaunchArgs {
        stream_url: resolved.master_url,
        // anidb's streams carry no referer requirement — 5.0's own
        // player invocation dropped the flag with the provider change.
        referer: None,
        title: Some(format!("{} · ep {}", args.title, args.episode)),
        player_command: cfg.external_player,
        player_kind: cfg.external_player_kind,
        custom_args_template: Some(cfg.external_player_custom_args),
    };
    external_player::open_external_player(&launch)
}

/// Fresh native resolution shared by the external-player and Syncplay
/// handoffs: walk the provider, feed the breaker the outcome, and
/// write the user's history under the slug — these surfaces are
/// always clicks, and the subprocess they replace wrote ani-hsts
/// itself. No progress sink: neither handoff has a loading overlay.
pub(super) async fn resolve_fresh_for_handoff(
    state: &AppState,
    args: &PlayArgs,
) -> Result<NativeResolved> {
    let client = anidb_client(state)?;
    let request = NativeResolveRequest {
        title: &args.title,
        alt_titles: &args.alt_titles,
        episode: &args.episode,
        mode: &args.mode,
        expected_count: args.episode_count,
        year: args.year,
        subtype: args.subtype.as_deref(),
    };
    let started_at = tokio::time::Instant::now();
    let mut sink = |_p: crate::anicli::parser::ProgressLine| {};
    let native = resolve_native(
        &client,
        Some(&state.scraper_gate),
        crate::scraper::gate::ScrapePriority::Interactive,
        request,
        &mut sink,
    )
    .await;
    state
        .scraper_gate
        .record(native_gate_outcome(&native), started_at);
    let native = native.map_err(|ne| ne.error)?;
    let entry = crate::history::HistoryEntry {
        ep_no: args.episode.clone(),
        id: native.slug.clone(),
        title: native.title.clone(),
    };
    if let Err(e) = crate::history::upsert_and_write(&state.history_path, entry) {
        tracing::warn!(
            title = %args.title,
            episode = %args.episode,
            error = ?e,
            "external handoff: history write failed after native resolve",
        );
    }
    Ok(native)
}

#[cfg(all(test, unix))]
#[path = "play_external_command_test.rs"]
mod tests;
