//! Cache-shaped helpers extracted from `commands::play` so that
//! file's cyclomatic complexity stays under the CRAP ratchet. Three
//! related helpers live here:
//!
//!   • `upstream_head_ok` — HEAD-pings a cached upstream URL with
//!     the right Referer and treats 2xx/3xx as live, anything else
//!     (including network errors) as dead.
//!   • `cached_track_ok` — reads a cached sidecar track the way the
//!     relay reads it: a GET with the Referer, live only when it
//!     answers 2xx and its first bytes carry the WebVTT signature.
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
use crate::commands::play_resolution_cache::{self, CachedResolution};
use crate::commands::session::{
    create_session_with_kind, CreateSessionArgs, CreateSessionResponse,
};

/// HEAD-validate that `url` is still alive, with the supplied
/// `referer` (empty string means "no Referer header"). 2xx and 3xx
/// (CDN edge redirects) both count as live; everything else,
/// including network errors, is dead.
pub(crate) async fn upstream_head_ok(
    client: &reqwest::Client,
    url: &url::Url,
    referer: &str,
) -> bool {
    let mut req = client.head(url.as_str());
    if !referer.is_empty() {
        req = req.header(reqwest::header::REFERER, referer);
    }
    let Ok(resp) = req.send().await else {
        return false;
    };
    resp.status().is_success() || resp.status().is_redirection()
}

/// What the first bytes of a body say about it being a WebVTT
/// track: `Some(true)` once they carry the signature — behind a
/// UTF-8 byte-order mark or not, as [`crate::proxy::is_webvtt`]
/// allows — `Some(false)` once they cannot, and `None` while too few
/// have arrived to tell either way. A decided prefix agrees with the
/// whole body's verdict.
#[must_use]
pub(crate) fn webvtt_prefix(bytes: &[u8]) -> Option<bool> {
    const BOM: &[u8] = b"\xEF\xBB\xBF";
    const SIGNATURE: &[u8] = b"WEBVTT";
    if BOM.starts_with(bytes) {
        // Empty, or still inside what may become a byte-order mark.
        return None;
    }
    let body = bytes.strip_prefix(BOM).unwrap_or(bytes);
    if body.len() >= SIGNATURE.len() {
        Some(body.starts_with(SIGNATURE))
    } else if SIGNATURE.starts_with(body) {
        None
    } else {
        Some(false)
    }
}

/// Whether a cached sidecar track is still a track: a GET with the
/// row's `referer` — what the relay sends when the player asks for
/// it — answers 2xx and its first bytes carry the WebVTT signature.
/// A HEAD is not enough: a CDN can answer one with 200 and serve a
/// challenge page to the GET, which the relay then refuses, and a
/// track that fails to load never reaches the player's recovery
/// path. The body is read only until its first bytes decide — a
/// chunk or two — and the response is dropped there, so the check
/// costs a request per track, not a track's worth of bytes.
pub(crate) async fn cached_track_ok(
    client: &reqwest::Client,
    url: &url::Url,
    referer: &str,
) -> bool {
    let mut req = client.get(url.as_str());
    if !referer.is_empty() {
        req = req.header(reqwest::header::REFERER, referer);
    }
    let Ok(mut resp) = req.send().await else {
        return false;
    };
    if !resp.status().is_success() {
        return false;
    }
    let mut head: Vec<u8> = Vec::new();
    loop {
        if let Some(verdict) = webvtt_prefix(&head) {
            return verdict;
        }
        match resp.chunk().await {
            Ok(Some(chunk)) => head.extend_from_slice(&chunk),
            // The body ended, or failed, before its first bytes
            // could say it is a track.
            Ok(None) | Err(_) => return false,
        }
    }
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
) -> Option<LaunchArgs> {
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
    Some(cached_launch_args(cached, args, cfg))
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
