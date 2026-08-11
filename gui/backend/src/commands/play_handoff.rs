//! The resolve half the two handoffs share.
//!
//! `play_external` and `play_syncplay` differ only in what they do
//! with a resolved stream — spawn the user's player, or spawn
//! Syncplay pointed at it. Getting the stream is the same work the
//! embedded player does, so it lives here once rather than twice.

use crate::app::AppState;
use crate::commands::external_player::LaunchArgs;
use crate::commands::play::{anidb_client_for, PlayArgs};
use crate::commands::play_native_resolve::{resolve_native_bounded, NativeResolveRequest};
use crate::config::read_config;
use crate::error::Result;

/// Resolve `args` through the native walk and describe the launch.
///
/// A handoff is always a click, never a prefetch, so the walk runs at
/// interactive priority, the breaker hears the resolution's outcome
/// under the same mapping the embedded path uses, and the watch is
/// recorded: sending an episode to mpv or Syncplay is as much a watch
/// as playing it in the window, and Continue Watching should show it
/// either way. A failed history write is logged and swallowed — the
/// stream is resolved and the user is waiting on their player.
///
/// # Errors
/// The walk's typed verdicts — `NoResults` for a clean miss, the
/// transport's own errors for weather.
pub async fn resolve_launch_args(state: &AppState, args: &PlayArgs) -> Result<LaunchArgs> {
    let quality = args.quality.as_deref().unwrap_or("best");
    let cfg = read_config(&state.config_path).unwrap_or_default();
    let prio = crate::scraper::gate::ScrapePriority::Interactive;
    let client = anidb_client_for(state, prio)?;
    let request = NativeResolveRequest {
        title: &args.title,
        alt_titles: &args.alt_titles,
        episode: &args.episode,
        mode: &args.mode,
        quality,
        expected_count: args.episode_count,
        year: args.year,
        subtype: args.subtype.as_deref(),
    };
    let started_at = tokio::time::Instant::now();
    let native = resolve_native_bounded(&client, request, &mut |_| {}).await;
    if let Some(outcome) = crate::commands::play_native_outcome::breaker_outcome(prio, &native) {
        let observed_at = native
            .as_ref()
            .err()
            .and_then(|ne| ne.failed_at)
            .or_else(|| client.transport().last_attempt_at())
            .unwrap_or(started_at);
        state.anidb_gate.record(outcome, observed_at);
    }
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
            "handoff: history write failed after native resolve",
        );
    }
    Ok(LaunchArgs {
        stream_url: native.master_url,
        // anidb's streams carry no referer requirement — the
        // embedded path records the same where it sets this empty.
        referer: None,
        title: Some(format!("{} · ep {}", args.title, args.episode)),
        player_command: cfg.external_player,
        player_kind: cfg.external_player_kind,
        custom_args_template: Some(cfg.external_player_custom_args),
    })
}
