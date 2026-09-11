//! The resolve half the two handoffs share.
//!
//! `play_external` and `play_syncplay` differ only in what they do
//! with a resolved stream — spawn the user's player, or spawn
//! Syncplay pointed at it. Getting the stream is the same work the
//! embedded player does, so it lives here once rather than twice.

use crate::app::AppState;
use crate::commands::external_player::LaunchArgs;
use crate::commands::play::PlayArgs;
use crate::commands::play_native_resolve::{NativeResolveRequest, NativeResolved};
use crate::config::{read_config, Config};
use crate::error::Result;

/// Resolve `args` through the native walk and describe the launch.
///
/// A handoff is always a click, never a prefetch, so the walk runs at
/// interactive priority against the providers in order, and each
/// breaker hears its attempt's outcome under the same mapping the
/// embedded path uses. The resolve stamps the numbering it learned
/// and hands back the watch to record — the history row and the
/// watched-at stamp — which the command writes once the player has
/// started: the spawn is the watch, and a player that fails to start
/// leaves nothing behind. Both paths go through
/// [`crate::commands::play_native_record`] so they cannot disagree
/// about what a row's number means.
///
/// # Errors
/// The walk's typed verdicts — `NoResults` for a clean miss, the
/// transport's own errors for weather.
pub async fn resolve_launch_args(
    state: &AppState,
    args: &PlayArgs,
) -> Result<(LaunchArgs, crate::commands::play_native_record::Watch)> {
    let quality = args.quality.as_deref().unwrap_or("best");
    let cfg = read_config(&state.config_path).unwrap_or_default();
    let prio = crate::scraper::gate::ScrapePriority::Interactive;
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
    let remembered = args
        .kitsu_id
        .as_deref()
        .and_then(|id| crate::commands::availability::cached_provider(state, id, &args.mode));
    let generation = crate::commands::availability_refresh::generation_at_start(
        &state.availability_refreshes,
        args.kitsu_id.as_deref(),
        &args.mode,
    );
    let mut attempt = crate::commands::providers::ResolveAttempt {
        request,
        on_progress: &mut |_| {},
        answered_by: None,
    };
    // The verdict is stamped as the embedded player stamps its own:
    // a served stream names the provider that served it, a clean
    // miss the provider whose miss it is.
    let native =
        match crate::commands::providers::run_from(state, remembered, prio, &mut attempt).await {
            Ok(attempted) => {
                crate::commands::availability::stamp_after_native(
                    state,
                    args.kitsu_id.as_deref(),
                    &args.mode,
                    generation,
                    crate::commands::availability::ResolveVerdict::served(
                        attempted.provider,
                        attempted.value.episode_cap,
                        &attempted.value.extra_tags,
                    ),
                )
                .await;
                attempted.value
            }
            Err(ne) => {
                if ne.clean_miss {
                    crate::commands::availability::stamp_after_native(
                        state,
                        args.kitsu_id.as_deref(),
                        &args.mode,
                        generation,
                        crate::commands::availability::ResolveVerdict::missed(attempt.answered_by),
                    )
                    .await;
                }
                return Err(ne.error);
            }
        };
    crate::commands::play_native_record::stamp_numbering(state, &native);
    let watch = crate::commands::play_native_record::Watch::of(&native);
    Ok((launch_args_for(native, args, &cfg), watch))
}

/// The launch a resolve describes: the stream, the referer its
/// provider named, the title, and the user's player settings.
pub(crate) fn launch_args_for(native: NativeResolved, args: &PlayArgs, cfg: &Config) -> LaunchArgs {
    LaunchArgs {
        stream_url: native.master_url,
        referer: native.referer,
        title: Some(format!("{} · ep {}", args.title, args.episode)),
        player_command: cfg.external_player.clone(),
        player_kind: cfg.external_player_kind,
        custom_args_template: Some(cfg.external_player_custom_args.clone()),
        subtitle_urls: subtitle_urls_default_first(&native.subtitles),
    }
}

/// The tracks' URLs with the provider's default first: a player that
/// takes one subtitle file takes the first listed. The provider's
/// order stands among the rest, and among several defaults.
#[must_use]
pub fn subtitle_urls_default_first(
    tracks: &[crate::scraper::provider::SubtitleTrack],
) -> Vec<String> {
    let (defaults, rest): (Vec<_>, Vec<_>) = tracks.iter().partition(|t| t.default);
    defaults
        .into_iter()
        .chain(rest)
        .map(|t| t.url.clone())
        .collect()
}
