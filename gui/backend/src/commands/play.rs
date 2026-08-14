//! Play action — bridges a Kitsu-resolved title to the actual stream.
//!
//! The renderer's detail page calls `POST /api/play` (or its sibling
//! `/api/play/external`) with the canonical title + episode + mode.
//! Resolution is native: the anidb client searches, disambiguates by
//! episode count, and walks episode → languages → embed →
//! master-playlist URL, which becomes a [`StreamSession`]. The
//! external and download siblings take the same walk and differ only
//! in what they do with the result.
//!
//! Resolved streams are cached. `play_with_progress` consults
//! `play_resolution_cache` first and returns a hit without touching
//! the provider; see that module for what the row holds and when it
//! is evicted.

use serde::Deserialize;

use crate::app::AppState;
use crate::commands::availability_refresh::with_row_if_ours;
use crate::commands::play_native_resolve::NativeResolveRequest;
use crate::commands::play_resolution_cache::{self, CachedResolution};
use crate::commands::progress::ProgressLine;
use crate::commands::session::{
    create_session_with_kind, CreateSessionArgs, CreateSessionResponse,
};
use crate::error::{AniError, Result};
use crate::proxy::MediaKind;

/// Frontend → backend payload for both play endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct PlayArgs {
    /// Canonical title from the Kitsu metadata. Fed to the provider's
    /// search step (after we've picked the right candidate index).
    pub title: String,
    /// Episode number, as a string to match the CLI's positional arg
    /// shape (`-e 5` accepts `"5"` literally).
    pub episode: String,
    /// `"sub"` or `"dub"`.
    pub mode: String,
    /// `"best"` / `"worst"` / `"1080"` / etc. Defaults to `"best"`.
    #[serde(default)]
    pub quality: Option<String>,
    /// Kitsu's authoritative episode count. Used to disambiguate
    /// allanime candidates that share a title (e.g. the 1-ep
    /// "Konoha Gakuen Den" side-story vs. the 500-ep main "Naruto:
    /// Shippuuden"). When `None`, we fall back to the legacy `-S 1`
    /// behaviour.
    #[serde(default)]
    pub episode_count: Option<u32>,
    /// Year the show first aired, parsed from Kitsu's `start_date`
    /// (`"1995-04-07"` → `1995`). The disambiguator uses it as the
    /// primary tie-break against allmanga's `airedStart.year` — much
    /// more discriminative than ep-count for franchise-overlap cases
    /// (Mobile Suit Gundam 1979 vs Gundam Wing 1995). `None` when the
    /// caller doesn't know the year (legacy SSE path, prefetch
    /// without metadata, etc.); picker degrades gracefully to pure
    /// ep-count + threshold.
    #[serde(default)]
    pub year: Option<u32>,
    /// Kitsu's subtype (`TV`, `movie`, `special`, `OVA`, `ONA`).
    /// Format disproof against the provider's card badges — a Movie
    /// candidate cannot satisfy a special/TV entry. Optional so old
    /// clients keep working.
    #[serde(default)]
    pub subtype: Option<String>,
    /// Fallback titles to try when the canonical title returns no
    /// allanime hits. Frontend feeds Kitsu's `titles.en_jp` /
    /// `titles.ja_jp` here so the play flow can recover when Kitsu's
    /// canonicalTitle is the English form (e.g. "JoJo's Bizarre
    /// Adventure: Stone Ocean") but allmanga only indexes the
    /// romanized name. Tried in order.
    ///
    /// Wire formats accepted (driven by `deserialize_alt_titles`):
    /// - JSON array (POST /api/play body): `["a","b"]`
    /// - Newline-joined string (SSE GET /api/play/stream query): `"a\nb"`.
    ///   Required because EventSource is GET-only and serde_urlencoded
    ///   doesn't handle repeated keys.
    #[serde(
        default,
        deserialize_with = "crate::commands::play_args::deserialize_alt_titles"
    )]
    pub alt_titles: Vec<String>,
    /// `true` when this call is a background prefetch (warming the
    /// cache for an episode the user hasn't clicked yet). Prefetches
    /// must NOT touch the history file — the page-mount loop fires
    /// 12+ play calls in parallel and whichever resolves last would
    /// overwrite the user's actual click. The flag skips both the
    /// cache-hit and the fresh-resolve history writes.
    ///
    /// Frontend prefetch loops set it; click handlers leave it false.
    #[serde(
        default,
        deserialize_with = "crate::commands::play_args::deserialize_loose_bool"
    )]
    pub prefetch: bool,
    /// Kitsu id of the anime the user is playing. The frontend knows
    /// it (the user came from `/anime/[kitsu_id]`); we don't, until
    /// the user passes it in. Recording the
    /// (allmanga show_id → kitsu_id) pair on every successful play
    /// turns the home-page Continue Watching lookup from "fuzzy
    /// kitsuSearch on a possibly-typo'd allmanga title" into a
    /// deterministic id-keyed lookup. Empty string when the caller
    /// has no kitsu_id available (e.g. the SSE fallback path or a
    /// direct API user).
    #[serde(default)]
    pub kitsu_id: Option<String>,
}

// `deserialize_alt_titles`, `deserialize_loose_bool`, and the

/// Update Continue Watching on a play-cache hit.
///
/// A cache miss records the row on its way through
/// `play_native_record::write_history`. A hit never reaches that
/// path, so the row is written here instead. No-op on prefetch calls
/// (the page-mount loop fires concurrently and whichever finishes
/// last would otherwise overwrite the user's real click) and on
/// legacy rows missing show_id.
fn write_history_on_cache_hit(state: &AppState, args: &PlayArgs, cached: &CachedResolution) {
    if args.prefetch || cached.show_id.is_empty() {
        return;
    }
    // the history file speaks the provider's numbering; the offset was
    // stamped by the fresh resolve that wrote this cache row.
    let offset = crate::commands::anidb_offset::get(state, &cached.show_id);
    let entry = crate::history::HistoryEntry {
        // The row's own slot when it carries one; older rows fall
        // back to the stamp-aware translation.
        ep_no: cached.resolved_slot.map_or_else(
            || {
                crate::commands::anidb_offset::write_ep_no(
                    state,
                    &cached.show_id,
                    &args.episode,
                    offset,
                )
            },
            |slot| slot.to_string(),
        ),
        id: cached.show_id.clone(),
        title: cached.show_title.clone(),
    };
    if let Err(e) = crate::history::upsert_and_write(&state.history_path, entry) {
        tracing::warn!(
            title = %args.title,
            episode = %args.episode,
            error = ?e,
            "play: history write failed on cache hit",
        );
    }
}

/// Stamp the availability cache with a native resolution's verdict,
/// guarded against a refresh that answered while the resolution was
/// in flight — the generation was captured before the resolve, and a
/// write over a newer answer would disable (or falsely enable) a show
/// the user was just told about.
pub(super) async fn stamp_availability_after_native(
    state: &AppState,
    args: &PlayArgs,
    available: bool,
    generation_at_start: u64,
    episode_cap: Option<u32>,
    extra_tags: &[String],
) {
    let Some(id) = args.kitsu_id.as_deref().filter(|s| !s.is_empty()) else {
        return;
    };
    let row = crate::commands::availability::cache_key(id, args.mode.as_str());
    with_row_if_ours(
        &state.availability_refreshes,
        &row,
        generation_at_start,
        false,
        || match episode_cap {
            // The resolve already paid for the provider's episode
            // list — the cap is exact FOR SUB, and dropping it would
            // evict an exact row into episode_count: null for the
            // whole TTL. A dub resolve only proves the requested
            // episode has an English embed, so the provider-wide
            // list must not become an exact (kitsu_id, dub) count —
            // the dub row stays boolean and self-heals via the next
            // mode-aware probe. Status is unknown at this call site,
            // so the row takes the ongoing TTL like the boolean
            // write.
            Some(cap) if available && args.mode != "dub" => {
                crate::commands::availability::write_cache_full(
                    state,
                    id,
                    &args.mode,
                    None,
                    &crate::commands::availability::AvailabilityResponse {
                        available: true,
                        episode_count: Some(cap),
                        // Derived from the listing the resolve paid
                        // for — the same tags a fractional play
                        // matches against number2, so they outrank
                        // whatever an older probe stored.
                        extra_episodes: extra_tags.to_vec(),
                        episode_count_approximate: false,
                        gate_refused: false,
                    },
                );
            }
            _ => crate::commands::availability::write_cache(state, id, &args.mode, available),
        },
    )
    .await;
}

/// The production anidb client: the curl-impersonate transport
/// resolved through the bundled directory then PATH, pointed at the
/// provider (or the test override), with every request admitted
/// through the scraper gate at `priority` — the walk fans out into
/// candidate probes and the episode chain, and each of those is a
/// provider request the pacing contract covers, not just the search.
///
/// # Errors
/// [`AniError::Network`] when no curl binary resolves at all — the
/// host cannot reach the provider by any transport.
pub(crate) fn anidb_client_for<'a>(
    state: &'a AppState,
    priority: crate::scraper::gate::ScrapePriority,
) -> Result<
    crate::scraper::anidb::AnidbClient<
        crate::scraper::anidb::GatedFetch<'a, crate::scraper::anidb::CurlImpersonateFetch>,
    >,
> {
    anidb_client_with_base(state, state.anidb_base.as_deref(), priority)
}

fn anidb_client<'a>(
    state: &'a AppState,
    priority: crate::scraper::gate::ScrapePriority,
) -> Result<
    crate::scraper::anidb::AnidbClient<
        crate::scraper::anidb::GatedFetch<'a, crate::scraper::anidb::CurlImpersonateFetch>,
    >,
> {
    anidb_client_with_base(state, state.anidb_base.as_deref(), priority)
}

/// [`anidb_client`] with an explicit origin override, shared with the
/// availability and download paths (and their test seams).
pub(super) fn anidb_client_with_base<'a>(
    state: &'a AppState,
    base: Option<&str>,
    priority: crate::scraper::gate::ScrapePriority,
) -> Result<
    crate::scraper::anidb::AnidbClient<
        crate::scraper::anidb::GatedFetch<'a, crate::scraper::anidb::CurlImpersonateFetch>,
    >,
> {
    let path_env = std::env::var("PATH").unwrap_or_default();
    let fetch = crate::scraper::anidb::CurlImpersonateFetch::resolve(
        state.bundled_bin.as_deref(),
        &path_env,
    )
    .ok_or_else(|| {
        tracing::error!("no curl binary found for the anidb transport");
        AniError::Network
    })?;
    let fetch = crate::scraper::anidb::GatedFetch::new(fetch, Some(&state.anidb_gate), priority);
    Ok(match base {
        Some(base) => crate::scraper::anidb::AnidbClient::with_base(fetch, base),
        None => crate::scraper::anidb::AnidbClient::new(fetch),
    })
}

/// Scraper-gate priority for a play-shaped request: prefetches (and
/// availability probes flagged background, which arrive here with
/// `prefetch = true` on their synthesized view) are opportunistic;
/// everything else is a user waiting.
fn scrape_priority(args: &PlayArgs) -> crate::scraper::gate::ScrapePriority {
    if args.prefetch {
        crate::scraper::gate::ScrapePriority::Background
    } else {
        crate::scraper::gate::ScrapePriority::Interactive
    }
}

/// Resolve `args` against the provider, register a stream session for the
/// resulting upstream URL, and return the proxy URLs hls.js will
/// consume.
///
/// # Errors
/// Inherits from the native resolve (timeout, parse failure, provider
/// errors) and [`create_session`] (URL-shape validation on the
/// resolved upstream).
pub async fn play(state: &AppState, args: &PlayArgs) -> Result<CreateSessionResponse> {
    play_with_progress(state, args, |_| {}).await
}

/// Like [`play`], but invokes `on_progress` once for every parsed
/// progress line as the resolution runs. Used by the SSE
/// `/api/play/stream` endpoint to forward incremental status to the
/// renderer's loading overlay.
///
/// The callback runs on the same async task as the resolution; a slow
/// callback stalls the subprocess. SSE handlers should push events
/// through an `mpsc` channel inside the callback rather than do work
/// inline.
///
/// # Errors
/// Same as [`play`].
pub async fn play_with_progress<F>(
    state: &AppState,
    args: &PlayArgs,
    mut on_progress: F,
) -> Result<CreateSessionResponse>
where
    F: FnMut(ProgressLine) + Send,
{
    let quality = args.quality.as_deref().unwrap_or("best");

    // Long-term cache check. A successful prior resolution under the
    // same (title, mode, quality, episode) tuple is replayable for up
    // to PLAY_RESOLUTION_TTL — we just have to confirm the upstream
    // URL is still alive (wixmp / sharepoint URLs rotate). HEAD is
    // ~50ms; a full resolve is ~30s. Worth the round-trip.
    let cache_key = play_resolution_cache::cache_key(
        &args.title,
        &args.mode,
        quality,
        &args.episode,
        args.year,
        args.episode_count,
        args.subtype.as_deref(),
    );
    if let Ok(Some(cached)) = play_resolution_cache::get(&state.cache_pool, &cache_key) {
        if let Some(resp) = try_serve_cached(state, &cached).await {
            tracing::info!(
                title = %args.title,
                episode = %args.episode,
                upstream = cached.upstream_url.as_str(),
                "play: cache hit (HEAD ok)",
            );
            write_history_on_cache_hit(state, args, &cached);
            return Ok(resp);
        }
        // HEAD failed — the cached URL is dead. Evict the row and
        // fall through to a fresh resolve. Eviction is explicit (not
        // just overwrite-on-put) because if the fresh resolve ALSO
        // fails, we don't want the stale row to linger and bite the
        // next attempt.
        play_resolution_cache::evict(&state.cache_pool, &cache_key);
        tracing::info!(
            title = %args.title,
            episode = %args.episode,
            "play: cache row stale (HEAD failed), evicted, resolving afresh",
        );
    }

    // Resolve natively against anidb: alias walk, bounded probing,
    // episode-to-master resolution. The subprocess never runs on the
    // play path — the -S index handoff it required is the coupling
    // the provider change broke.
    // Captured before the resolving, not before the write: the answer
    // a play resolution stamps is the one it got here, and a refresh
    // can land any time between.
    let availability_generation = crate::commands::availability_refresh::generation_at_start(
        &state.availability_refreshes,
        args.kitsu_id.as_deref(),
        args.mode.as_str(),
    );
    let client = anidb_client(state, scrape_priority(args))?;
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
    let resolve_started_at = tokio::time::Instant::now();
    // Bounded: a provider that accepts connections but stalls every
    // request must not keep the play pending past the gate's
    // half-open trial window (see RESOLVE_DEADLINE).
    let native = crate::commands::play_native_resolve::resolve_native_bounded(
        &client,
        request,
        &mut on_progress,
    )
    .await;
    // Feed the breaker the resolution's outcome so background traffic
    // backs off after provider-shaped failures — and only those. The
    // mapping lives in play_native_resolve::breaker_outcome: answered
    // verdicts (clean misses, absent episodes or audio) are health,
    // weather is distress.
    // None = the gate refused before any provider contact; the
    // breaker only hears about requests that got past it.
    if let Some(outcome) =
        crate::commands::play_native_outcome::breaker_outcome(scrape_priority(args), &native)
    {
        // Timestamped with the attempt that OBSERVED the outcome,
        // not the chain's start: the gate's stale filters discard
        // evidence predating the last recovery, and a long resolve
        // would otherwise have its fresh 429 thrown away whenever a
        // concurrent resolve recorded recovery mid-chain. The chain
        // start remains the fallback for a resolve refused before
        // any fetch ran.
        let observed_at = native
            .as_ref()
            .err()
            .and_then(|ne| ne.failed_at)
            .or_else(|| client.transport().last_attempt_at())
            .unwrap_or(resolve_started_at);
        state.anidb_gate.record(outcome, observed_at);
    }
    let native = match native {
        Ok(n) => n,
        Err(ne) => {
            if ne.clean_miss {
                // The one verdict that proves absence — persist it,
                // guarded against a refresh that answered meanwhile.
                stamp_availability_after_native(
                    state,
                    args,
                    false,
                    availability_generation,
                    None,
                    &[],
                )
                .await;
            }
            tracing::info!(
                title = %args.title,
                episode = %args.episode,
                clean_miss = ne.clean_miss,
                error = ?ne.error,
                "play: native resolution failed",
            );
            return Err(ne.error);
        }
    };
    // A successful resolve is a positive availability fact, same
    // guard — carrying the episode cap the native episodes list
    // already paid for.
    stamp_availability_after_native(
        state,
        args,
        true,
        availability_generation,
        native.episode_cap,
        &native.extra_tags,
    )
    .await;
    crate::commands::play_native_record::stamp_numbering(state, &native);
    // Prefetches stay out of the user's history exactly as before.
    if !args.prefetch {
        crate::commands::play_native_record::write_history(state, &native, &args.episode);
    }

    let upstream_url = url::Url::parse(&native.master_url).map_err(|_| AniError::ParseFailed {
        detail: format!("upstream_url: {} is not a valid URL", native.master_url),
    })?;

    // anidb's streams carry no referer requirement — 5.0's own player
    // invocation dropped the flag with the provider change.
    let referer = String::new();

    // The resolve already fetched this URL and accepted it only
    // because its body opens as #EXTM3U — it is definitively HLS,
    // whatever shape the URL takes and however the CDN answers HEAD.
    // Re-deriving the kind here misclassified opaque playlist URLs
    // as MP4 and probed the stream upstream through the metadata
    // client.
    let kind = MediaKind::Hls;
    tracing::info!(
        title = %args.title,
        episode = %args.episode,
        upstream = upstream_url.as_str(),
        referer = referer.as_str(),
        kind = ?kind,
        "play: natively resolved upstream",
    );

    // Persist the resolution so the next play of the same episode
    // skips the provider round-trips (subject to TTL + HEAD
    // validation). show_id is the anidb slug, and storing it is what
    // lets a cache hit write the same history row a fresh resolve
    // would: `play_native_record::write_history` has the slug in hand
    // from the walk, `write_history_on_cache_hit` reads it back from
    // here. Nothing outside this app writes that file.
    let cached_resolution = CachedResolution {
        upstream_url: native.master_url.clone(),
        referer: referer.clone(),
        media_kind: kind,
        show_id: native.slug.clone(),
        show_title: native.title.clone(),
        resolved_slot: Some(native.resolved_slot),
    };
    play_resolution_cache::put(&state.cache_pool, &cache_key, &cached_resolution);

    let session_args = CreateSessionArgs {
        upstream_url: native.master_url,
        referer,
    };
    create_session_with_kind(state, &session_args, kind)
}

// `upstream_head_ok`, `try_serve_cached`, and
// `try_launch_args_from_cache` live in `commands::play_cache` so
// this module's reported CCN stays under the CRAP ratchet's
// per-file limit. The tests in this file's `#[cfg(test)]` module
// still drive them via wiremock; they just import from the new
// module rather than calling sibling functions.
#[cfg(test)]
use crate::commands::play_cache::try_launch_args_from_cache;
use crate::commands::play_cache::try_serve_cached;

#[cfg(test)]
mod tests {
    use super::*;

    proptest::proptest! {
        // The gate's lane assignment is exactly the prefetch bit:
        // warms ride Background pacing, everything a user waits on
        // rides Interactive — no other PlayArgs field may influence
        // it.
        #[test]
        fn scrape_priority_maps_exactly_from_the_prefetch_bit(
            prefetch in proptest::bool::ANY,
            title in ".*",
            episode in "[0-9]{1,4}",
            mode in "(sub|dub|weird)",
            quality in proptest::option::of("[a-z0-9]{1,6}"),
            episode_count in proptest::option::of(proptest::num::u32::ANY),
            year in proptest::option::of(proptest::num::u32::ANY),
            kitsu_id in proptest::option::of("[a-z0-9-]{1,12}"),
        ) {
            let args = PlayArgs {
                title,
                episode,
                mode,
                quality,
                episode_count,
                subtype: None,
                year,
                alt_titles: vec![],
                prefetch,
                kitsu_id,
            };
            let got = scrape_priority(&args);
            let want = if prefetch {
                crate::scraper::gate::ScrapePriority::Background
            } else {
                crate::scraper::gate::ScrapePriority::Interactive
            };
            proptest::prop_assert_eq!(got, want);
        }
    }

    /// Build an `AppState` for the `try_serve_cached` tests. Mirrors
    /// `app::tests::fake_state` (private, unreachable from here) so the
    /// shape stays in lock-step.
    fn state_with_proxy_origin() -> AppState {
        use crate::meta::kitsu::KitsuClient;
        use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
        use std::sync::Arc;
        AppState {
            // Unroutable: the fresh-resolve fallback must fail fast in
            // tests instead of walking the live provider. Windows
            // runners ship a system curl.exe, so a None base turns
            // "the fallback errors" into a real network resolve that
            // can succeed.
            anidb_base: Some("http://127.0.0.1:1".into()),
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            meta_http: reqwest::Client::new(),
            proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
            bundled_bin: None,
            legacy_sweep: crate::legacy_script::SweepReport::default(),
            history_path: std::path::PathBuf::from("/tmp/ani-gui/history"),
            anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
            image_cache_dir: std::path::PathBuf::from("/tmp/ani-gui-images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: KitsuClient::new(reqwest::Client::new()),
            config_path: std::path::PathBuf::from("/tmp/ani-gui-config.toml"),
            state_dir: std::path::PathBuf::from("/tmp/ani-gui-state"),
            internal_secret: crate::account::InternalSecret::random(),
            mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
            account_write_locks: crate::commands::account::AccountWriteLocks::new(),
            availability_refreshes:
                crate::commands::availability_refresh::AvailabilityRefreshes::new(),
        }
    }

    /// The Codex-flagged gap: when no availability row exists — the
    /// mount-time probe failed, or the show was never probed — a
    /// successful play stamped extra_episodes: [], hiding every
    /// decimal episode for the row's whole TTL. The resolve already
    /// paid for the provider's full listing, and its fractional
    /// display tags are exactly what the strip needs to advertise.
    #[tokio::test]
    async fn the_exact_cap_stamp_derives_extras_with_no_prior_row() {
        let state = state_with_proxy_origin();
        let args = PlayArgs {
            title: "x".into(),
            episode: "3".into(),
            mode: "sub".into(),
            quality: None,
            subtype: None,
            episode_count: None,
            year: None,
            alt_titles: vec![],
            prefetch: false,
            kitsu_id: Some("42".into()),
        };
        let generation = crate::commands::availability_refresh::generation_at_start(
            &state.availability_refreshes,
            args.kitsu_id.as_deref(),
            args.mode.as_str(),
        );
        stamp_availability_after_native(
            &state,
            &args,
            true,
            generation,
            Some(1061),
            &["1061.5".to_string()],
        )
        .await;
        let key = crate::commands::availability::cache_key("42", "sub");
        let body = crate::cache::meta_cache_get(&state.cache_pool, &key)
            .expect("cache read")
            .expect("row present");
        let row: crate::commands::availability::AvailabilityResponse =
            serde_json::from_str(&body).expect("row parses");
        assert_eq!(
            row.extra_episodes,
            vec!["1061.5".to_string()],
            "the stamp must advertise the listing's own fractional tags"
        );
        assert_eq!(row.episode_count, Some(1061));
    }

    /// The listing the stamp derives from is the same one fractional
    /// plays resolve against, so its tags outrank whatever an older
    /// probe stored: a stale extra the provider no longer lists is a
    /// dead tile at play time, and preserving it would keep the
    /// corpse alive for the row's TTL.
    #[tokio::test]
    async fn the_exact_cap_stamp_replaces_stale_extras_with_the_listings() {
        let state = state_with_proxy_origin();
        crate::commands::availability::write_cache_full(
            &state,
            "42",
            "sub",
            None,
            &crate::commands::availability::AvailabilityResponse {
                available: true,
                episode_count: Some(1060),
                extra_episodes: vec!["999.5".into()],
                episode_count_approximate: false,
                gate_refused: false,
            },
        );
        let args = PlayArgs {
            title: "x".into(),
            episode: "3".into(),
            mode: "sub".into(),
            quality: None,
            subtype: None,
            episode_count: None,
            year: None,
            alt_titles: vec![],
            prefetch: false,
            kitsu_id: Some("42".into()),
        };
        let generation = crate::commands::availability_refresh::generation_at_start(
            &state.availability_refreshes,
            args.kitsu_id.as_deref(),
            args.mode.as_str(),
        );
        stamp_availability_after_native(
            &state,
            &args,
            true,
            generation,
            Some(1061),
            &["1061.5".to_string()],
        )
        .await;
        let key = crate::commands::availability::cache_key("42", "sub");
        let body = crate::cache::meta_cache_get(&state.cache_pool, &key)
            .expect("cache read")
            .expect("row present");
        let row: crate::commands::availability::AvailabilityResponse =
            serde_json::from_str(&body).expect("row parses");
        assert_eq!(
            row.extra_episodes,
            vec!["1061.5".to_string()],
            "the freshly derived tags replace the stale cached ones"
        );
        assert_eq!(row.episode_count, Some(1061));
    }

    /// Three consecutive clean misses (the show simply isn't in the
    /// catalogue — a fresh Continue Watching rail full of uncarried
    /// titles produces exactly this) must not open the breaker: a
    /// clean miss is the provider answering, not the provider
    /// failing. Only transport errors, refusals and rate limits are
    /// distress.
    #[tokio::test]
    async fn a_clean_miss_streak_does_not_open_the_breaker() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_string(r#"<div class="grid"><p>No results.</p></div>"#),
            )
            .mount(&server)
            .await;
        let mut state = state_with_proxy_origin();
        state.anidb_base = Some(server.uri());
        let mk = |title: &str| PlayArgs {
            title: title.into(),
            episode: "1".into(),
            mode: "sub".into(),
            quality: None,
            subtype: None,
            episode_count: None,
            year: None,
            alt_titles: vec![],
            prefetch: false,
            kitsu_id: None,
        };
        for i in 0..crate::scraper::gate::FAILURE_THRESHOLD + 1 {
            let r = play_with_progress(&state, &mk(&format!("absent show {i}")), |_| {}).await;
            assert!(r.is_err(), "an empty catalogue cannot resolve");
        }
        assert!(
            state
                .anidb_gate
                .admit(crate::scraper::gate::ScrapePriority::Background)
                .await
                .is_ok(),
            "breaker must stay closed through clean misses"
        );
    }

    /// Build a CachedResolution with the new show_id/show_title fields
    /// defaulted to empty (so try_serve_cached's history-write skip
    /// branch fires). Tests that want history-write coverage override
    /// the two fields explicitly.
    fn cached_blank(upstream_url: String, referer: String, kind: MediaKind) -> CachedResolution {
        CachedResolution {
            upstream_url,
            referer,
            media_kind: kind,
            show_id: String::new(),
            show_title: String::new(),
            resolved_slot: None,
        }
    }

    #[test]
    fn cache_hit_history_writes_the_providers_numbering() {
        // The history file speaks the provider's numbering — a reader greps
        // the stored ep_no in the provider's episode list — so a
        // Kitsu-relative "1" written for a continuation cour (whose
        // provider list starts at 41) vanishes from the resume list.
        // The writer adds the offset stamped at resolve time.
        let tmp = tempfile::tempdir().unwrap();
        let mut state = state_with_proxy_origin();
        state.history_path = tmp.path().join("history");
        let mut cached = cached_blank(
            "https://cdn.example/x/master.m3u8".into(),
            String::new(),
            MediaKind::Hls,
        );
        cached.show_id = "the-sequel-88".into();
        cached.show_title = "The Sequel".into();
        crate::commands::anidb_offset::put(&state, "the-sequel-88", 40);
        let args = PlayArgs {
            title: "The Sequel".into(),
            episode: "1".into(),
            mode: "sub".into(),
            quality: None,
            subtype: None,
            episode_count: None,
            year: None,
            alt_titles: vec![],
            prefetch: false,
            kitsu_id: None,
        };
        write_history_on_cache_hit(&state, &args, &cached);
        let rows = crate::history::read_all(&state.history_path).expect("rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ep_no, "41", "provider numbering, not Kitsu's");
    }

    #[tokio::test]
    async fn try_serve_cached_returns_none_when_url_is_unparseable() {
        // A corrupt cache row with garbage in upstream_url shouldn't
        // crash — fall through to a fresh resolve.
        let state = state_with_proxy_origin();
        let cached = cached_blank(
            "not://a valid url at all".into(),
            String::new(),
            MediaKind::Mp4,
        );
        assert!(try_serve_cached(&state, &cached).await.is_none());
    }

    #[tokio::test]
    async fn try_serve_cached_returns_session_on_2xx_head() {
        // Cache hit happy path: upstream HEAD returns 200 → we register
        // a session and return its CreateSessionResponse. This is the
        // ~50ms path that replaces the ~30s resolve.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .and(wiremock::matchers::path("/video.mp4"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let cached = cached_blank(
            format!("{}/video.mp4", server.uri()),
            String::new(),
            MediaKind::Mp4,
        );
        let resp = try_serve_cached(&state, &cached).await.expect("hit");
        // Session is freshly created, but the upstream + kind match.
        assert!(resp.media_url.contains("/file.mp4"));
        assert_eq!(resp.media_kind, MediaKind::Mp4);
        // The cache_hit flag is what tells the renderer whether a
        // player error is silently retryable. Cache-served responses
        // must set it; the post-resolve path must not.
        assert!(
            resp.cache_hit,
            "try_serve_cached must tag the response so the renderer can retry on player error"
        );
    }

    #[tokio::test]
    async fn try_serve_cached_returns_none_on_404() {
        // Stale wixmp URL — HEAD 404 means the row is dead. Return
        // None so the caller resolves afresh (which will
        // overwrite the row with a fresh resolution).
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(wiremock::ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let cached = cached_blank(
            format!("{}/expired.mp4", server.uri()),
            String::new(),
            MediaKind::Mp4,
        );
        assert!(try_serve_cached(&state, &cached).await.is_none());
    }

    #[tokio::test]
    async fn try_serve_cached_sends_referer_header_when_set() {
        // fast4speed.rsvp upstreams 403 without `Referer:
        // https://allmanga.to`. The cached referer must round-trip
        // through the HEAD validation; otherwise the row appears dead
        // even when it isn't.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .and(wiremock::matchers::header("referer", "https://allmanga.to"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let cached = cached_blank(
            format!("{}/sub/1", server.uri()),
            "https://allmanga.to".into(),
            MediaKind::Mp4,
        );
        assert!(try_serve_cached(&state, &cached).await.is_some());
    }

    fn external_args(title: &str, episode: &str) -> PlayArgs {
        PlayArgs {
            title: title.into(),
            episode: episode.into(),
            mode: "sub".into(),
            quality: Some("best".into()),
            subtype: None,
            episode_count: None,
            year: None,
            alt_titles: vec![],
            prefetch: false,
            kitsu_id: None,
        }
    }

    fn external_cfg() -> crate::config::Config {
        crate::config::Config {
            external_player: "test-player".into(),
            ..Default::default()
        }
    }

    fn seed_play_cache(state: &AppState, args: &PlayArgs, upstream: &str, referer: &str) {
        let key = play_resolution_cache::cache_key(
            &args.title,
            &args.mode,
            args.quality.as_deref().unwrap_or("best"),
            &args.episode,
            args.year,
            args.episode_count,
            args.subtype.as_deref(),
        );
        play_resolution_cache::put(
            &state.cache_pool,
            &key,
            &CachedResolution {
                upstream_url: upstream.into(),
                referer: referer.into(),
                media_kind: MediaKind::Mp4,
                show_id: "abc".into(),
                show_title: "Test (12 episodes)".into(),
                resolved_slot: None,
            },
        );
    }

    /// Drive `play_with_progress` through the cache-hit short-circuit
    /// so the lines inside the `if let Some(cached) = ...` branch
    /// (history-write skip, info!, the early `return Ok(resp)`) all
    /// run. This is a real test of the embedded-player fast path —
    /// it would have caught the regression that prompted the
    /// long-term cache to ship.
    #[tokio::test]
    async fn play_with_progress_returns_cache_hit_response_when_head_succeeds() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let args = external_args("Cached Show", "5");
        let upstream = format!("{}/cached.mp4", server.uri());
        seed_play_cache(&state, &args, &upstream, "");
        let resp = play_with_progress(&state, &args, |_| {})
            .await
            .expect("cache-hit returns Ok");
        assert!(
            resp.cache_hit,
            "play_with_progress must tag cache-hit responses so the renderer can retry on player error"
        );
        assert_eq!(resp.media_kind, MediaKind::Mp4);
    }

    /// A still-valid cache row for a fractional extra can outlive
    /// the sidecar's single (slot, tag) stamp — resolving 1.5 then
    /// 2.5 replaces the stamp, and the 1.5 replay would fall back
    /// to the display tag a history reader cannot grep. The cache row itself
    /// carries the resolved slot, so the replay writes it directly.
    #[test]
    fn cache_hit_history_writes_the_cached_slot() {
        let state = state_with_proxy_origin();
        let td = tempfile::tempdir().expect("tempdir");
        let mut state = state;
        state.history_path = td.path().join("history");
        let args = PlayArgs {
            title: "Recap Show".into(),
            episode: "3.5".into(),
            mode: "sub".into(),
            quality: None,
            subtype: None,
            episode_count: None,
            year: None,
            alt_titles: vec![],
            prefetch: false,
            kitsu_id: None,
        };
        let mut cached = cached_blank(
            "https://u.example/x.mp4".into(),
            String::new(),
            MediaKind::Mp4,
        );
        cached.show_id = "recap-show-7".into();
        cached.show_title = "Recap Show".into();
        cached.resolved_slot = Some(4);
        write_history_on_cache_hit(&state, &args, &cached);
        let body = std::fs::read_to_string(&state.history_path).unwrap_or_default();
        assert!(
            body.contains("\t4\t") || body.starts_with("4\t"),
            "the replay must write the cached slot, not the display tag; got {body:?}"
        );
        assert!(
            !body.contains("3.5"),
            "the display tag must not reach the shared file; got {body:?}"
        );
    }

    /// Same shape, but with a non-empty referer + show_id — exercises
    /// the cache-hit history-write branch (lines 266-282 in the file
    /// before this test landed). Without this the upsert-on-cache-hit
    /// path was uncovered, leaving Continue Watching's "I just played
    /// this" feedback silently broken if it regressed.
    #[tokio::test]
    async fn play_with_progress_writes_history_on_cache_hit_with_show_id() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let args = external_args("Show With History", "3");
        let upstream = format!("{}/cached.mp4", server.uri());
        seed_play_cache(&state, &args, &upstream, "");
        // Non-prefetch click → history must be written. The upsert
        // target is state.history_path, which is /tmp/ani-gui/history
        // by default — make it a real tempfile so the write
        // succeeds and we can assert against it.
        let td = tempfile::tempdir().expect("tempdir");
        let mut state = state;
        state.history_path = td.path().join("history");
        let _ = play_with_progress(&state, &args, |_| {}).await.expect("ok");
        // The history file must exist with one row referencing the
        // seeded show_id.
        let body = std::fs::read_to_string(&state.history_path).unwrap_or_default();
        assert!(
            body.contains("abc"),
            "history must contain seeded show_id; got: {body:?}"
        );
    }

    /// HEAD failure → cache row evicted, function falls through to
    /// a fresh resolve (which fails because the provider is unreachable
    /// in the test fixture). The test just needs to confirm the
    /// eviction-and-fallthrough branch runs without panicking;
    /// covers lines 288-292 (eviction warn).
    #[tokio::test]
    async fn play_with_progress_evicts_cache_when_head_fails_then_returns_error() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(wiremock::ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let args = external_args("Stale Show", "1");
        let upstream = format!("{}/dead.mp4", server.uri());
        seed_play_cache(&state, &args, &upstream, "");
        let r = play_with_progress(&state, &args, |_| {}).await;
        assert!(
            r.is_err(),
            "the fresh-resolve fallback must error in the test env"
        );
        // Cache row should be gone.
        let key = play_resolution_cache::cache_key(
            &args.title,
            &args.mode,
            "best",
            &args.episode,
            args.year,
            args.episode_count,
            None,
        );
        assert!(
            play_resolution_cache::get(&state.cache_pool, &key)
                .ok()
                .flatten()
                .is_none(),
            "stale row must be evicted on HEAD failure"
        );
    }

    #[tokio::test]
    async fn try_launch_args_from_cache_returns_none_on_cache_miss() {
        let state = state_with_proxy_origin();
        let args = external_args("Never Played", "1");
        let cfg = external_cfg();
        assert!(try_launch_args_from_cache(&state, &args, &cfg)
            .await
            .is_none());
    }

    #[tokio::test]
    async fn try_launch_args_from_cache_returns_launch_args_on_2xx_head() {
        // Happy path — cache hit + HEAD ok → caller can hand the
        // returned LaunchArgs to mpv without resolving again.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let args = external_args("Naruto", "5");
        seed_play_cache(&state, &args, &format!("{}/v.mp4", server.uri()), "");
        let cfg = external_cfg();

        let launch = try_launch_args_from_cache(&state, &args, &cfg)
            .await
            .expect("hit");

        assert!(launch.stream_url.contains("/v.mp4"));
        assert!(
            launch.referer.is_none(),
            "empty cached referer must round-trip as None"
        );
        assert_eq!(launch.player_command, "test-player");
        assert_eq!(launch.title.as_deref(), Some("Naruto · ep 5"));
    }

    #[tokio::test]
    async fn try_launch_args_from_cache_evicts_and_returns_none_on_404() {
        // Stale upstream — HEAD 404. The cache row must be evicted so a
        // fresh resolve will overwrite, AND we return None so the
        // caller falls through to the fresh path.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(wiremock::ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let args = external_args("Stale", "1");
        let upstream = format!("{}/dead.mp4", server.uri());
        seed_play_cache(&state, &args, &upstream, "");
        let cfg = external_cfg();

        let result = try_launch_args_from_cache(&state, &args, &cfg).await;
        assert!(result.is_none());

        // Cache row should be gone; a fresh attempt would re-resolve.
        let key = play_resolution_cache::cache_key(
            &args.title,
            &args.mode,
            "best",
            &args.episode,
            args.year,
            args.episode_count,
            None,
        );
        assert!(
            play_resolution_cache::get(&state.cache_pool, &key)
                .ok()
                .flatten()
                .is_none(),
            "stale cache row must be evicted on HEAD failure"
        );
    }

    #[tokio::test]
    async fn try_launch_args_from_cache_round_trips_the_referer() {
        // Signed-URL upstreams need the cached Referer header
        // forwarded to the external player.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .and(wiremock::matchers::header("referer", "https://allmanga.to"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let args = external_args("Fast4", "3");
        let key = play_resolution_cache::cache_key(
            &args.title,
            &args.mode,
            "best",
            &args.episode,
            args.year,
            args.episode_count,
            None,
        );
        play_resolution_cache::put(
            &state.cache_pool,
            &key,
            &CachedResolution {
                upstream_url: format!("{}/ep/3", server.uri()),
                referer: "https://allmanga.to".into(),
                media_kind: MediaKind::Mp4,
                show_id: "x".into(),
                show_title: "Fast4 (12 episodes)".into(),
                resolved_slot: None,
            },
        );
        let cfg = external_cfg();

        let launch = try_launch_args_from_cache(&state, &args, &cfg)
            .await
            .expect("hit");
        assert_eq!(launch.referer.as_deref(), Some("https://allmanga.to"));
    }

    #[tokio::test]
    async fn try_launch_args_from_cache_returns_none_on_unparseable_url() {
        let state = state_with_proxy_origin();
        let args = external_args("Bad URL", "1");
        seed_play_cache(&state, &args, "not://a valid url", "");
        let cfg = external_cfg();
        assert!(try_launch_args_from_cache(&state, &args, &cfg)
            .await
            .is_none());
    }

    #[test]
    fn play_args_quality_defaults_to_best() {
        let args = PlayArgs {
            title: "test".into(),
            episode: "1".into(),
            mode: "sub".into(),
            quality: None,
            subtype: None,
            episode_count: None,
            year: None,
            alt_titles: vec![],
            prefetch: false,
            kitsu_id: None,
        };
        assert_eq!(args.quality.as_deref().unwrap_or("best"), "best");
    }

    #[test]
    fn play_args_alt_titles_default_to_empty_when_omitted() {
        // Older clients (and `/api/play/external` callers that don't
        // know about the field yet) send the JSON without alt_titles.
        // Serde default keeps that path working — the play flow still
        // runs with just the canonical title.
        let json = r#"{"title":"x","episode":"1","mode":"sub"}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert!(args.alt_titles.is_empty());
    }

    #[test]
    fn play_args_deserializes_alt_titles_when_present() {
        let json = r#"{"title":"JoJo's Bizarre Adventure: Stone Ocean","episode":"1","mode":"sub","alt_titles":["Jojo no Kimyou na Bouken Part 6: Stone Ocean","ジョジョの奇妙な冒険 ストーンオーシャン"]}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert_eq!(args.alt_titles.len(), 2);
        assert_eq!(
            args.alt_titles[0],
            "Jojo no Kimyou na Bouken Part 6: Stone Ocean"
        );
    }

    #[test]
    fn play_args_deserializes_alt_titles_from_newline_joined_query_string() {
        // SSE GET path — EventSource can't POST, and serde_urlencoded
        // can't deserialize Vec<String> from repeated keys. The frontend
        // joins alt_titles with `\n` for this path; backend splits.
        let qs = "title=Stone+Ocean&episode=1&mode=sub&alt_titles=a%0Ab%0Ac";
        let args: PlayArgs = serde_urlencoded::from_str(qs).expect("parses");
        assert_eq!(args.alt_titles, vec!["a", "b", "c"]);
    }

    #[test]
    fn play_args_treats_empty_alt_titles_string_as_empty_vec() {
        // The frontend sends `alt_titles=` for shows whose Kitsu titles
        // map is empty (rare but real). Backend must still parse.
        let qs = "title=X&episode=1&mode=sub&alt_titles=";
        let args: PlayArgs = serde_urlencoded::from_str(qs).expect("parses");
        assert!(args.alt_titles.is_empty());
    }

    /// Pass a literal `null` for alt_titles so the deserializer's
    /// `None` arm fires (serde's `default` only short-circuits when
    /// the FIELD is missing; an explicit `null` still goes through
    /// `deserialize_alt_titles`).
    #[test]
    fn play_args_treats_explicit_null_alt_titles_as_empty_vec() {
        let json = r#"{"title":"x","episode":"1","mode":"sub","alt_titles":null}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert!(args.alt_titles.is_empty());
    }

    /// `prefetch` tolerates the JSON bool form, the SSE-string form
    /// ("1" / "true" / "yes"), and missing / null. Test all three
    /// truthy strings + the negative ones + null so the
    /// `deserialize_loose_bool` switch is fully exercised.
    #[test]
    fn play_args_loose_bool_accepts_true_strings() {
        for truthy in ["1", "true", "yes"] {
            let qs = format!("title=X&episode=1&mode=sub&prefetch={truthy}");
            let args: PlayArgs = serde_urlencoded::from_str(&qs).expect("parses");
            assert!(args.prefetch, "expected prefetch=true for {truthy:?}");
        }
    }

    #[test]
    fn play_args_loose_bool_treats_other_strings_as_false() {
        for falsy in ["0", "false", "no", "wat"] {
            let qs = format!("title=X&episode=1&mode=sub&prefetch={falsy}");
            let args: PlayArgs = serde_urlencoded::from_str(&qs).expect("parses");
            assert!(!args.prefetch, "expected prefetch=false for {falsy:?}");
        }
    }

    #[test]
    fn play_args_loose_bool_accepts_explicit_json_bool() {
        // Direct POST clients still send the field as a JSON
        // boolean — serde_json's untagged enum tries the Bool arm
        // first.
        let json = r#"{"title":"x","episode":"1","mode":"sub","prefetch":true}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert!(args.prefetch);
    }

    #[test]
    fn play_args_loose_bool_treats_explicit_null_as_false() {
        // Pin the None-arm of `deserialize_loose_bool` — explicit
        // `null` should keep the click-path default rather than
        // erroring.
        let json = r#"{"title":"x","episode":"1","mode":"sub","prefetch":null}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert!(!args.prefetch);
    }

    #[test]
    fn play_args_prefetch_defaults_to_false_when_omitted() {
        // Older clients (and click handlers that don't bother passing
        // the field) leave prefetch implicit — must default to false
        // so the history-write path stays active for clicks.
        let json = r#"{"title":"x","episode":"1","mode":"sub"}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert!(!args.prefetch);
    }

    #[test]
    fn play_args_prefetch_accepts_json_bool() {
        let json = r#"{"title":"x","episode":"1","mode":"sub","prefetch":true}"#;
        let args: PlayArgs = serde_json::from_str(json).expect("parses");
        assert!(args.prefetch);
    }

    #[test]
    fn play_args_prefetch_accepts_query_string_one() {
        // SSE GET path: serde_urlencoded can't decode bool directly.
        // The custom deserializer handles "1" / "true" / "yes" / "0".
        let qs = "title=X&episode=1&mode=sub&prefetch=1";
        let args: PlayArgs = serde_urlencoded::from_str(qs).expect("parses");
        assert!(args.prefetch);
    }

    #[test]
    fn play_args_prefetch_zero_string_means_false() {
        let qs = "title=X&episode=1&mode=sub&prefetch=0";
        let args: PlayArgs = serde_urlencoded::from_str(qs).expect("parses");
        assert!(!args.prefetch);
    }

    #[tokio::test]
    async fn a_native_stamp_leaves_the_row_alone_when_a_refresh_answered_first() {
        // The generation is captured before the resolve; the whole
        // provider round-trip sits between it and the write. A user
        // re-ask that answers in that window must win — a stale
        // negative stamped over it disables a show the user was just
        // told about.
        let state = std::sync::Arc::new(state_with_proxy_origin());
        let args = PlayArgs {
            title: "Race Show".into(),
            episode: "1".into(),
            mode: "sub".into(),
            quality: None,
            subtype: None,
            episode_count: None,
            year: None,
            alt_titles: vec![],
            prefetch: false,
            kitsu_id: Some("race-1".into()),
        };
        let row = crate::commands::availability::cache_key("race-1", "sub");
        let generation = crate::commands::availability_refresh::generation_at_start(
            &state.availability_refreshes,
            Some("race-1"),
            "sub",
        );

        // The refresh answers first: bump + a positive write.
        state.availability_refreshes.bump(&row);
        crate::commands::availability::write_cache(&state, "race-1", "sub", true);

        // The stale resolution now tries to stamp a negative.
        stamp_availability_after_native(&state, &args, false, generation, None, &[]).await;

        let cached = crate::commands::availability::batch_cached(
            &state,
            &crate::commands::availability::AvailabilityBatchArgs {
                kitsu_ids: vec!["race-1".into()],
                mode: "sub".into(),
            },
        );
        assert_eq!(
            cached.cached.get("race-1"),
            Some(&true),
            "the refresh's answer must survive the stale stamp"
        );
    }

    #[tokio::test]
    async fn a_native_success_stamp_persists_the_resolved_cap() {
        // The resolve already paid for the provider's episode list;
        // a boolean-only stamp evicts an exact availability row into
        // episode_count: null, so home resume and episode prefetch
        // fall back to Kitsu's announced total for the row's whole
        // TTL and target episodes the provider has not listed.
        let state = std::sync::Arc::new(state_with_proxy_origin());
        let args = PlayArgs {
            title: "Cap Show".into(),
            episode: "1".into(),
            mode: "sub".into(),
            quality: None,
            subtype: None,
            episode_count: None,
            year: None,
            alt_titles: vec![],
            prefetch: false,
            kitsu_id: Some("cap-1".into()),
        };
        let generation = crate::commands::availability_refresh::generation_at_start(
            &state.availability_refreshes,
            Some("cap-1"),
            "sub",
        );
        stamp_availability_after_native(&state, &args, true, generation, Some(2), &[]).await;
        let cached = crate::commands::availability::batch_cached(
            &state,
            &crate::commands::availability::AvailabilityBatchArgs {
                kitsu_ids: vec!["cap-1".into()],
                mode: "sub".into(),
            },
        );
        assert_eq!(cached.cached.get("cap-1"), Some(&true));
        assert_eq!(
            cached.playable_episode_counts.get("cap-1"),
            Some(&2),
            "the resolved cap must survive the stamp"
        );
    }

    #[tokio::test]
    async fn a_dub_success_stamp_keeps_the_cap_out_of_the_row() {
        // picked.episodes is the provider-wide list: a dub resolve
        // only proves the REQUESTED episode has an English embed.
        // Persisting the total-list cap as an exact (kitsu_id, dub)
        // count makes undubbed later episodes look playable for the
        // row's whole TTL. The dub stamp stays boolean-only; the
        // count self-heals via the next mode-aware probe.
        let state = std::sync::Arc::new(state_with_proxy_origin());
        let args = PlayArgs {
            title: "Dub Show".into(),
            episode: "1".into(),
            mode: "dub".into(),
            quality: None,
            subtype: None,
            episode_count: None,
            year: None,
            alt_titles: vec![],
            prefetch: false,
            kitsu_id: Some("dub-1".into()),
        };
        let generation = crate::commands::availability_refresh::generation_at_start(
            &state.availability_refreshes,
            Some("dub-1"),
            "dub",
        );
        stamp_availability_after_native(&state, &args, true, generation, Some(12), &[]).await;
        let cached = crate::commands::availability::batch_cached(
            &state,
            &crate::commands::availability::AvailabilityBatchArgs {
                kitsu_ids: vec!["dub-1".into()],
                mode: "dub".into(),
            },
        );
        assert_eq!(cached.cached.get("dub-1"), Some(&true));
        assert!(
            !cached.playable_episode_counts.contains_key("dub-1"),
            "the provider-wide cap must not become an exact dub count"
        );
    }
}
