//! Cache-shaped helpers extracted from `commands::play` so that
//! file's cyclomatic complexity stays under the CRAP ratchet. Three
//! related helpers live here:
//!
//!   • `cached_row_is_live` — the check both replays share: the
//!     row's stream passes `upstream_head_ok` and every sidecar
//!     track it lists passes `cached_track_ok`, with the row's
//!     referer, asked together under one deadline.
//!   • `try_serve_cached` — turns a `CachedResolution` row into a
//!     fresh session response when the row is live. Used by the
//!     embedded-player flow's fast path in `play_with_progress`.
//!   • `try_launch_args_from_cache` — sibling of `try_serve_cached`
//!     for the external-player flow: walks the same cache, checks
//!     the row the same way, and returns ready-to-launch
//!     [`LaunchArgs`] (or `None`) so `play_external` can hand mpv a
//!     cached URL without resolving again.
//!   • `stamp_availability_on_cache_hit` — the availability refresh
//!     both replays owe: the provider the cached row's show key
//!     names is stamped on the request's row, as a fresh resolve
//!     would have stamped it.
//!
//! The readers a row's check is made of — the stream's HEAD ping,
//! the WebVTT prefix, the track's GET — live in
//! `commands::play_cache_tracks` and are re-exported here.
//!
//! All three are async and depend on `AppState`'s reqwest client +
//! cache pool, so the fixtures in play.rs's test module
//! (`state_with_proxy_origin`, `cached_blank`, `seed_play_cache`)
//! are still used to drive them via wiremock.
//!
//! No behaviour change relative to the previous in-`play.rs`
//! definitions — pure relocation to drop the file's reported CCN.

use crate::app::AppState;
use crate::commands::external_player::LaunchArgs;
use crate::commands::play_cache_tracks::{cached_track_ok, upstream_head_ok};
use crate::commands::play_resolution_cache::{self, CachedResolution};
use crate::commands::session::{
    create_session_with_kind, CreateSessionArgs, CreateSessionResponse,
};
use crate::scraper::provider::{ProviderId, ShowKey};

/// The provider a cached row was resolved through: the one its show
/// key names — a qualified key for any provider but anidb, whose
/// keys are bare. A row from before the field carries an empty key
/// and names no provider.
pub(crate) fn cached_row_provider(cached: &CachedResolution) -> Option<ProviderId> {
    if cached.show_id.is_empty() {
        return None;
    }
    Some(ShowKey::parse(&cached.show_id).provider)
}

/// Refresh the availability row a served replay stands on with the
/// provider the cached row names, for the request's own show and
/// mode — the embedded player's replay and the handoff's alike, so
/// the affinity a fresh resolve would have written survives the
/// resolution cache outliving the row.
///
/// `at_start` is the row as the caller read it BEFORE the liveness
/// check went out — the same ordering a fresh resolve keeps — so a
/// refresh that lands while the check is in flight owns the row and
/// the replay's count-less positive does not overwrite the exact
/// verdict it just wrote.
pub(crate) async fn stamp_availability_on_cache_hit(
    state: &AppState,
    args: &super::play::PlayArgs,
    cached: &CachedResolution,
    at_start: &crate::commands::availability::RowAtStart,
) {
    let Some(provider) = cached_row_provider(cached) else {
        return;
    };
    crate::commands::availability::stamp_after_cache_hit(
        state,
        args.kitsu_id.as_deref(),
        args.mode.as_str(),
        at_start,
        provider,
    )
    .await;
}

/// The row a play's stamp is ordered against — its refresh
/// generation, its count of positive writes and the provider it
/// remembers — read before the liveness check, never after it.
pub(crate) async fn row_before_check(
    state: &AppState,
    args: &super::play::PlayArgs,
) -> crate::commands::availability::RowAtStart {
    crate::commands::availability::RowAtStart::read(
        state,
        args.kitsu_id.as_deref(),
        args.mode.as_str(),
    )
    .await
}

/// How long a cached row may take to prove itself live. The row is
/// a shortcut past a fresh resolve, whose first request answers in
/// a few seconds; a shortcut that takes longer than that is no
/// shortcut, and the metadata client would otherwise let each URL
/// stall for its own thirty seconds. The URLs are asked together,
/// so this bounds the whole check, however many tracks the row
/// lists.
pub(crate) const CACHED_ROW_CHECK_DEADLINE: std::time::Duration =
    std::time::Duration::from_secs(10);

/// Whether a cached row can still be served: its stream answers the
/// HEAD check, and every sidecar track it lists reads as a track
/// ([`cached_track_ok`]) — each with the row's referer, which is
/// what the relay sends when it fetches a track. A track is signed
/// like the stream and expires on its own, and a track that fails
/// to load never reaches the player's recovery path, so a row is
/// live only when everything it names is. The URLs are asked
/// together under [`CACHED_ROW_CHECK_DEADLINE`]; the first dead URL
/// ends the check, a URL that does not parse is dead, and a deadline
/// that elapses is a row that is not live.
pub(crate) async fn cached_row_is_live(state: &AppState, cached: &CachedResolution) -> bool {
    cached_row_is_live_within(state, cached, CACHED_ROW_CHECK_DEADLINE).await
}

/// [`cached_row_is_live`] under a caller's deadline.
pub(crate) async fn cached_row_is_live_within(
    state: &AppState,
    cached: &CachedResolution,
    deadline: std::time::Duration,
) -> bool {
    let Ok(stream_url) = url::Url::parse(&cached.upstream_url) else {
        return false;
    };
    let mut track_urls = Vec::with_capacity(cached.subtitles.len());
    for track in &cached.subtitles {
        let Ok(url) = url::Url::parse(&track.url) else {
            return false;
        };
        track_urls.push(url);
    }
    let stream = async {
        upstream_head_ok(&state.meta_http, &stream_url, &cached.referer)
            .await
            .then_some(())
            .ok_or(())
    };
    let tracks = track_urls.iter().map(|url| async move {
        cached_track_ok(&state.meta_http, url, &cached.referer)
            .await
            .then_some(())
            .ok_or(())
    });
    let all = futures_util::future::try_join(stream, futures_util::future::try_join_all(tracks));
    matches!(tokio::time::timeout(deadline, all).await, Ok(Ok(_)))
}

/// Serve a cached row as a fresh CreateSessionResponse when the row
/// is live ([`cached_row_is_live`]), or `None` when its stream or one
/// of its tracks is dead, unreachable or answers an error status —
/// the caller falls through to a fresh resolve.
pub(crate) async fn try_serve_cached(
    state: &AppState,
    cached: &CachedResolution,
) -> Option<CreateSessionResponse> {
    if !cached_row_is_live(state, cached).await {
        return None;
    }
    let session_args = CreateSessionArgs {
        subtitles: cached.subtitles.clone(),
        upstream_url: cached.upstream_url.clone(),
        referer: cached.referer.clone(),
    };
    let mut resp = create_session_with_kind(state, &session_args, cached.media_kind).ok()?;
    // Tag so the renderer can decide whether a player error is
    // retryable (cache hit can be evicted + re-resolved) or terminal
    // (fresh fetch, no cache to clear).
    resp.cache_hit = true;
    Some(resp)
}

/// Cache-hit branch of `play_external` and `play_syncplay`: returns
/// ready-to-launch `LaunchArgs` when the play_resolution_cache has a
/// live row ([`cached_row_is_live`]), otherwise `None` (caller falls
/// through to a fresh resolve). A dead stream or a dead track evicts
/// the row before returning None so the next attempt isn't bitten by
/// the same dead URL.
pub(crate) async fn try_launch_args_from_cache(
    state: &AppState,
    args: &super::play::PlayArgs,
    cfg: &crate::config::Config,
) -> Option<(LaunchArgs, super::play_native_record::Watch)> {
    // The replay opt-out covers this surface too: with caching off
    // the row still exists (it carries the watch metadata), but no
    // playback path may replay its URL.
    if !cfg.cache_resolutions {
        return None;
    }
    let quality = args.quality.as_deref().unwrap_or("best");
    let cache_key = play_resolution_cache::cache_key(
        &args.title,
        &args.mode,
        quality,
        &args.episode,
        args.year,
        args.episode_count,
        args.subtype.as_deref(),
    );
    let cached = play_resolution_cache::get(&state.cache_pool, &cache_key).ok()??;
    // Captured before the check goes out, like a fresh resolve's
    // answer: a refresh landing during the check owns the row.
    let at_start = row_before_check(state, args).await;
    if !cached_row_is_live(state, &cached).await {
        play_resolution_cache::evict(&state.cache_pool, &cache_key);
        tracing::info!(
            title = %args.title,
            episode = %args.episode,
            "play_external: cache row stale (the stream's HEAD or a track's read failed), evicted, resolving afresh",
        );
        return None;
    }
    tracing::info!(
        title = %args.title,
        episode = %args.episode,
        upstream = cached.upstream_url.as_str(),
        "play_external: cache hit (stream and tracks live), launching mpv from cached URL",
    );
    stamp_availability_on_cache_hit(state, args, &cached, &at_start).await;
    let watch = cached_watch(state, &cached, &args.episode);
    Some((cached_launch_args(cached, args, cfg), watch))
}

/// The watch a cached resolution describes, for the command to
/// record once the player has started. The history file speaks the
/// provider's numbering: the row's own slot when it carries one —
/// the display number translated through the single display stamp
/// can point at a recap once a later resolve has moved the stamp —
/// and the stamp-aware translation only for a row from before the
/// field, as the embedded player's cache-hit path does.
pub(crate) fn cached_watch(
    state: &AppState,
    cached: &play_resolution_cache::CachedResolution,
    episode: &str,
) -> super::play_native_record::Watch {
    let ep_no = cached.resolved_slot.map_or_else(
        || {
            let offset = super::anidb_offset::get(state, &cached.show_id);
            super::anidb_offset::write_ep_no(state, &cached.show_id, episode, offset)
        },
        |slot| slot.to_string(),
    );
    super::play_native_record::Watch {
        show_id: cached.show_id.clone(),
        title: cached.show_title.clone(),
        ep_no,
    }
}

/// The launch a cached resolution describes: the row's stream and
/// referer, its sidecar tracks with the provider's default first —
/// as the fresh resolve lists them — and the user's player settings.
pub(crate) fn cached_launch_args(
    cached: play_resolution_cache::CachedResolution,
    args: &super::play::PlayArgs,
    cfg: &crate::config::Config,
) -> LaunchArgs {
    LaunchArgs {
        stream_url: cached.upstream_url,
        referer: if cached.referer.is_empty() {
            None
        } else {
            Some(cached.referer)
        },
        title: Some(format!("{} · ep {}", args.title, args.episode)),
        player_command: cfg.external_player.clone(),
        player_kind: cfg.external_player_kind,
        custom_args_template: Some(cfg.external_player_custom_args.clone()),
        subtitle_urls: super::play_handoff::subtitle_urls_default_first(&cached.subtitles),
    }
}

#[cfg(test)]
#[path = "play_cache_test.rs"]
mod tests;
