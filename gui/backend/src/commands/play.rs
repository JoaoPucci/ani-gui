//! Play action — bridges a Kitsu-resolved title to the actual stream.
//!
//! The renderer's detail page calls `POST /api/play` (or its sibling
//! `/api/play/external`) with the canonical title + episode + mode.
//! Both endpoints walk the same chain:
//!
//!   1. Spawn `ani-cli -S <title> -e <episode>` via [`run_debug`].
//!      ani-cli internally searches allanime, picks the first match,
//!      resolves the chosen quality stream, and prints the result.
//!   2. Take the parsed [`DebugOutput`] and either
//!        - wrap the upstream URL in a [`StreamSession`] (embedded),
//!        - or hand it off to the user's `mpv` (external).
//!
//! No title-match cache yet — every play hits ani-cli fresh. The cache
//! is task #51 and lands once the spawn cost actually bites the UX.

use std::time::Duration;

use serde::Deserialize;

use crate::anicli::parser::{parse_progress_line, ProgressLine};
use crate::anicli::process::{run_debug_streaming, DebugOptions};
use crate::app::AppState;
use crate::commands::play_resolution_cache::{self, CachedResolution};
use crate::commands::session::{
    create_session_with_kind, CreateSessionArgs, CreateSessionResponse,
};
use crate::error::{AniError, Result};
use crate::proxy::{upstream, MediaKind};
use crate::scraper;
use crate::scraper::Candidate;

/// Frontend → backend payload for both play endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct PlayArgs {
    /// Canonical title from the Kitsu metadata. Fed to ani-cli's
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
        deserialize_with = "crate::commands::play_select::deserialize_alt_titles"
    )]
    pub alt_titles: Vec<String>,
    /// `true` when this call is a background prefetch (warming the
    /// cache for an episode the user hasn't clicked yet). Prefetches
    /// must NOT touch `ani-hsts` — the page-mount loop fires 12+ play
    /// calls in parallel and whichever resolves last would overwrite
    /// the user's actual click. The flag drives both:
    ///   - skipping our cache-hit history write
    ///   - redirecting ani-cli's `$ANI_CLI_HIST_DIR` to a tempdir so
    ///     ani-cli's own `update_history` writes to a throwaway file
    ///
    /// Frontend prefetch loops set it; click handlers leave it false.
    #[serde(
        default,
        deserialize_with = "crate::commands::play_select::deserialize_loose_bool"
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
// `select_first_with_hits*` family live in `commands::play_select`
// so this module's cyclomatic complexity stays manageable. The
// PlayArgs serde derive uses fully-qualified paths above, and the
// rest of the play flow imports them via the `use` line at the top
// of the file.
pub use crate::commands::play_select::{
    select_first_with_hits, select_first_with_hits_opt, select_first_with_hits_with_candidate,
};

/// Spawn timeout for the ani-cli search+resolve step. Real-world
/// allanime queries take 5-30s; 60s is a comfortable upper bound
/// before the user is better served by an error than a stuck spinner.
const RUN_DEBUG_TIMEOUT: Duration = Duration::from_secs(60);

/// Resolve which `(title, 1-based candidate index)` to pass to
/// `ani-cli -S`. Calls our own allanime search for the canonical
/// title first; if that returns zero hits, walks `args.alt_titles`
/// in order until one returns a non-empty list, then runs
/// `pick_by_ep_count` over the winner. Falls through to
/// `(args.title, 1)` (legacy behaviour) on every-list-empty or when
/// `episode_count` is unknown.
/// Result of [`pick_title_and_index`]. Adds a transient-error
/// indicator alongside the picker's (title, index, candidate)
/// triple so callers can tell "we have no safe match" apart from
/// "we couldn't ask allmanga." A negative-availability cache write
/// is correct in the first case and wrong in the second — Codex
/// flagged the difference in P2 #3233589818.
pub(super) struct PickedTitle {
    pub title: String,
    pub index: usize,
    pub candidate: Option<Candidate>,
    /// True when at least one `scraper::search` call returned `Ok`
    /// (with any number of hits — including zero). False only when
    /// every search call hit a network / upstream / parse error.
    /// When this is false AND `candidate.is_none()`, callers should
    /// surface `AniError::Network` rather than `NoResults`.
    pub any_search_succeeded: bool,
    /// True when at least one `scraper::search` call returned `Err`.
    /// Distinct from `any_search_succeeded`: a run can have some
    /// errors AND some successes, in which case the verdict
    /// is incomplete — the canonical lookup may have failed while
    /// only an alt-title returned `Ok` with zero hits. Codex P2
    /// #3233658501. Callers should suppress availability cache
    /// writes (no `write_cache(false)`) when this is true even if
    /// `any_search_succeeded` is also true.
    pub any_search_errored: bool,
}

/// Update Continue Watching on a play-cache hit. The cache-miss
/// path gets history for free via ani-cli's `update_history`; we
/// don't run ani-cli on a hit, so write the row directly. No-op
/// on prefetch calls (the page-mount loop fires concurrently and
/// whichever finishes last would otherwise overwrite the user's
/// real click) and on legacy rows missing show_id.
fn write_history_on_cache_hit(state: &AppState, args: &PlayArgs, cached: &CachedResolution) {
    if args.prefetch || cached.show_id.is_empty() {
        return;
    }
    let entry = crate::history::HistoryEntry {
        ep_no: args.episode.clone(),
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

/// Stamp the availability cache after a successful ani-cli resolve.
/// One extra GraphQL round-trip (`fetch_show`) gets the truthful
/// episode cap + recap-extras list so the next detail/play visit
/// doesn't re-probe; failure falls through to the simple
/// `write_cache(true)` so the row at least records availability.
/// No-op when the caller didn't supply a kitsu_id.
async fn enrich_availability_after_success(
    state: &AppState,
    args: &PlayArgs,
    chosen_candidate: Option<&Candidate>,
) {
    let Some(id) = args.kitsu_id.as_deref().filter(|s| !s.is_empty()) else {
        return;
    };
    let Some(c) = chosen_candidate else {
        crate::commands::availability::write_cache(state, id, &args.mode, true);
        return;
    };
    let mode_str = args.mode.as_str();
    let detail = if state
        .scraper_gate
        .admit(scrape_priority(args))
        .await
        .is_ok()
    {
        let got = crate::scraper::allanime::fetch_show(&state.meta_http, &c.id, None).await;
        state.scraper_gate.record_outcome(got.is_ok());
        got.ok()
    } else {
        // Breaker open — skip the enrichment round-trip; the plain
        // write below still records availability.
        None
    };
    let (episode_count, extras) = match detail {
        Some(detail) => {
            let cap = detail.max_integer_episode(mode_str);
            let ex: Vec<String> = detail
                .available_episodes_detail
                .for_mode(mode_str)
                .iter()
                .filter(|t| t.parse::<u32>().is_err())
                .cloned()
                .collect();
            (cap, ex)
        }
        None => (None, Vec::new()),
    };
    // Status unknown at this layer (PlayArgs doesn't carry it).
    // None → write_cache_full uses the conservative ongoing TTL
    // (24h); the next detail-page probe knows status and will
    // overwrite with the right TTL.
    crate::commands::availability::write_cache_full(
        state,
        id,
        mode_str,
        true,
        episode_count,
        extras,
        None,
    );
}

/// Caller-side picker-miss error policy used by the sibling
/// `play_external` / `play_syncplay` / `download_with_progress`
/// surfaces. Returns `NoResults` only when every search call
/// completed (no errors) and still nothing matched — any error in
/// the mix is treated as transient and surfaced as `Network`.
/// Same policy as [`classify_picker_miss`] (which additionally
/// stamps the availability negative-cache on the all-completed
/// branch); the non-play callers don't touch that cache so this
/// pure variant is enough for them.
pub(super) fn picker_miss_caller_error(picked: &PickedTitle) -> AniError {
    if picked.any_search_succeeded && !picked.any_search_errored {
        AniError::NoResults
    } else {
        AniError::Network
    }
}

/// Map a "no chosen candidate" outcome to the right `AniError`,
/// optionally stamping the availability cache. Three branches —
/// see the comments inside for the policy. Extracted out of
/// `play_with_progress` to keep its ccn under the firm CRAP ceiling.
fn classify_picker_miss(state: &AppState, args: &PlayArgs, picked: &PickedTitle) -> AniError {
    if !picked.any_search_succeeded {
        tracing::warn!(
            kitsu_id = ?args.kitsu_id,
            "play: every allanime search errored; surfacing transient Network",
        );
        return AniError::Network;
    }
    if picked.any_search_errored {
        // Verdict is incomplete: the failed search may have been the
        // canonical with the right hit, so surface transient Network
        // (no negative cache write) and let the next attempt retry —
        // same policy as availability / download / play_external /
        // play_syncplay. Codex P2 #3236... .
        tracing::info!(
            search_title = %picked.title,
            kitsu_id = ?args.kitsu_id,
            "play: partial search failure + no safe match; surfacing Network",
        );
        return AniError::Network;
    }
    tracing::info!(
        search_title = %picked.title,
        kitsu_id = ?args.kitsu_id,
        "play: picker found no safe allmanga match; surfacing NoResults",
    );
    if let Some(id) = args.kitsu_id.as_deref().filter(|s| !s.is_empty()) {
        crate::commands::availability::write_cache(state, id, &args.mode, false);
    }
    AniError::NoResults
}

pub(super) async fn pick_title_and_index(state: &AppState, args: &PlayArgs) -> PickedTitle {
    pick_title_and_index_with_base(state, args, None).await
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

/// Admit the ani-cli spawn itself through the scraper gate for
/// prefetches. The picker's searches are admitted per request, but
/// the subprocess performs its own allanime traffic — background
/// prefetch spawns must be paced the same way and skipped while the
/// breaker is open. Interactive plays pass untouched.
///
/// # Errors
/// [`AniError::Network`] when the gate refuses a background admit.
async fn admit_prefetch_spawn(state: &AppState, args: &PlayArgs) -> Result<()> {
    if !args.prefetch {
        return Ok(());
    }
    state
        .scraper_gate
        .admit(crate::scraper::gate::ScrapePriority::Background)
        .await
        .map_err(|_| AniError::Network)
}

/// Feed a spawn's outcome back to the gate — every spawn, not just
/// prefetches: interactive plays bypass admission, but the preflight
/// search just reset the breaker with a success, so when the spawn
/// itself hits the rate limit background traffic must back off
/// instead of resuming right after a user-visible failure.
/// `NoResults` counts as a failure: the spawn only happens after the
/// picker confirmed the show exists on allanime, so the subprocess
/// finding nothing moments later is transient/upstream evidence — a
/// rate-limited ani-cli dies with exactly that message. `Scraper{}`
/// verdicts are content-level answers ("Episode not released" from
/// an ep+1 prefetch at the season edge, dep_ch complaints) and move
/// the breaker in neither direction.
/// Side effects of a failed ani-cli spawn: an explicit error log (so
/// `RUST_LOG=ani_gui=info` surfaces the actual reason instead of
/// leaving the user staring at an overlay that flashed and
/// disappeared — the `?` would propagate it but nothing between here
/// and the SSE serializer prints it) and the negative availability
/// write for NoResults clicks so home/search list filters learn from
/// the click without an extra round-trip.
fn note_spawn_failure(
    state: &AppState,
    args: &PlayArgs,
    search_title: &str,
    select_index: usize,
    e: &AniError,
) {
    tracing::error!(
        search_title = %search_title,
        episode = %args.episode,
        select_index = select_index,
        error = ?e,
        "play: ani-cli step failed",
    );
    if matches!(e, AniError::NoResults) {
        if let Some(id) = args.kitsu_id.as_deref().filter(|s| !s.is_empty()) {
            crate::commands::availability::write_cache(state, id, &args.mode, false);
        }
    }
}

fn record_spawn_outcome<T>(state: &AppState, result: &Result<T>) {
    match result {
        Ok(_) => state.scraper_gate.record_outcome(true),
        Err(AniError::Scraper { .. }) => {}
        Err(_) => state.scraper_gate.record_outcome(false),
    }
}

/// [`pick_title_and_index`] with the allanime endpoint override
/// exposed for tests. Production passes `None` via the wrapper.
pub(super) async fn pick_title_and_index_with_base(
    state: &AppState,
    args: &PlayArgs,
    allanime_base: Option<&str>,
) -> PickedTitle {
    let primary = args.title.clone();
    let mode = if args.mode == "dub" { "dub" } else { "sub" };

    // Walk the candidate list whether or not we have a Kitsu
    // episode_count to disambiguate with — alt_titles is also the
    // recovery path when canonical doesn't appear in allmanga's index
    // (Stone Ocean Part 6 reproduces this even though its
    // episode_count is null on Kitsu).
    //
    // We interleave fetch + pick: after each non-empty pool lands,
    // re-run the picker against the accumulated `results`. If it
    // accepts, we stop and skip the remaining alt-title GraphQL
    // calls (the common case for unambiguous canonical hits). If
    // the picker rejects (year mismatch, ep-count over tolerance),
    // we keep walking — the picker's None on this pool is what
    // makes "canonical returned only wrong-year siblings, but
    // romanized alt returned the real show" recoverable. Without
    // this, the early break on the first non-empty pool would lock
    // us into the wrong show. Codex P2 #3231391353.
    let mut results: Vec<(String, Vec<Candidate>)> = Vec::new();
    let mut chosen_so_far: Option<Candidate> = None;
    let mut chosen_title_so_far = primary.clone();
    let mut chosen_pick_so_far = 1usize;
    let mut any_search_succeeded = false;
    let mut any_search_errored = false;
    let prio = scrape_priority(args);
    for title in
        std::iter::once(args.title.as_str()).chain(args.alt_titles.iter().map(String::as_str))
    {
        // Background walks stop as soon as the breaker opens — the
        // remaining alt titles would fail identically and each doomed
        // request deepens allanime's rate limit.
        if state.scraper_gate.admit(prio).await.is_err() {
            tracing::warn!(title, "play: scraper gate open; abandoning candidate walk");
            any_search_errored = true;
            break;
        }
        match scraper::search(&state.meta_http, title, mode, allanime_base).await {
            Ok(cands) => {
                state.scraper_gate.record_outcome(true);
                tracing::info!(title, hits = cands.len(), "play: allanime search candidate",);
                results.push((title.to_string(), cands));
                any_search_succeeded = true;
            }
            Err(e) => {
                state.scraper_gate.record_outcome(false);
                tracing::warn!(
                    title,
                    error = ?e,
                    "play: allanime search failed; trying next candidate",
                );
                results.push((title.to_string(), Vec::new()));
                any_search_errored = true;
                continue;
            }
        }
        // Try to pick from what we have. If accepted, stop and skip
        // any remaining alt-title fetches. If still None, keep
        // walking — later alt titles may yield a valid candidate.
        let (t, p, c) = select_first_with_hits_with_candidate(
            &primary,
            &results,
            args.episode_count,
            args.year,
            mode,
        );
        if c.is_some() {
            chosen_so_far = c;
            chosen_title_so_far = t;
            chosen_pick_so_far = p;
            break;
        }
    }

    let (chosen_title, pick, chosen) = (chosen_title_so_far, chosen_pick_so_far, chosen_so_far);
    tracing::info!(
        primary = %primary,
        alt_count = args.alt_titles.len(),
        chosen_title = %chosen_title,
        expected_eps = ?args.episode_count,
        pick = pick,
        chosen_show_id = chosen.as_ref().map(|c| c.id.as_str()).unwrap_or(""),
        any_search_succeeded = any_search_succeeded,
        "play: chose ani-cli search title",
    );
    PickedTitle {
        title: chosen_title,
        index: pick,
        candidate: chosen,
        any_search_succeeded,
        any_search_errored,
    }
}

/// Build the spawn options for an ani-cli invocation. When
/// `override_hist_dir` is `Some`, ani-cli writes its `ani-hsts` to that
/// path instead of the user's real history file — used by the prefetch
/// path to keep background warming out of Continue Watching.
pub(super) fn debug_options_for(
    state: &AppState,
    override_hist_dir: Option<&std::path::Path>,
) -> DebugOptions {
    let hist_dir = override_hist_dir
        .map(std::path::Path::to_path_buf)
        .or_else(|| {
            state
                .history_path
                .parent()
                .map(std::path::Path::to_path_buf)
        });
    DebugOptions {
        ani_cli_path: state.ani_cli_path.clone(),
        bash_path: state.bash_path.clone(),
        bundled_bin: state.bundled_bin.clone(),
        hist_dir,
        timeout: RUN_DEBUG_TIMEOUT,
        // None → inherit the backend process's PATH. Tests inject a
        // shimmed PATH by calling `run_debug` directly with their own
        // `DebugOptions` rather than going through the play handlers.
        path_override: None,
    }
}

/// Resolve `args` against ani-cli, register a stream session for the
/// resulting upstream URL, and return the proxy URLs hls.js will
/// consume.
///
/// # Errors
/// Inherits from [`run_debug`] (timeout, parse failure, scraper
/// errors) and [`create_session`] (URL-shape validation on the
/// resolved upstream).
pub async fn play(state: &AppState, args: &PlayArgs) -> Result<CreateSessionResponse> {
    play_with_progress(state, args, |_| {}).await
}

/// Like [`play`], but invokes `on_progress` once for every parsed
/// `ani-cli` stderr line as the resolution runs. Used by the SSE
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
    // Per-call scratch dir for ani-cli's history write when this is a
    // prefetch — keeps background warming out of the user's real
    // ani-hsts. Held across the await so the dir lives until ani-cli
    // exits; auto-cleaned on drop.
    let prefetch_hist_dir = if args.prefetch {
        Some(tempfile::tempdir().map_err(|_| AniError::Io)?)
    } else {
        None
    };
    let opts = debug_options_for(state, prefetch_hist_dir.as_ref().map(|d| d.path()));
    let quality = args.quality.as_deref().unwrap_or("best");

    // Long-term cache check. A successful prior resolution under the
    // same (title, mode, quality, episode) tuple is replayable for up
    // to PLAY_RESOLUTION_TTL — we just have to confirm the upstream
    // URL is still alive (wixmp / sharepoint URLs rotate). HEAD is
    // ~50ms; ani-cli is ~30s. Worth the round-trip.
    let cache_key = play_resolution_cache::cache_key(
        &args.title,
        &args.mode,
        quality,
        &args.episode,
        args.year,
        args.episode_count,
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
        // fall through to ani-cli. Eviction is explicit (not just
        // overwrite-on-put) because if the fresh ani-cli call ALSO
        // fails, we don't want the stale row to linger and bite the
        // next attempt.
        play_resolution_cache::evict(&state.cache_pool, &cache_key);
        tracing::info!(
            title = %args.title,
            episode = %args.episode,
            "play: cache row stale (HEAD failed), evicted, falling back to ani-cli",
        );
    }

    // Pick which (title, candidate index) ani-cli should use. The title
    // may differ from args.title when alt_titles produced the winning
    // hit (e.g. romanized fallback for shows whose Kitsu canonicalTitle
    // is the English form). See pick_title_and_index().
    let picked = pick_title_and_index(state, args).await;
    if picked.candidate.is_none() {
        return Err(classify_picker_miss(state, args, &picked));
    }
    let search_title = picked.title;
    let select_index = picked.index;
    let chosen_candidate = picked.candidate;

    // The subprocess makes its own allanime requests: prefetch spawns
    // are background traffic and go through the gate (paced, refused
    // while the breaker is open). User clicks pass untouched.
    admit_prefetch_spawn(state, args).await?;

    tracing::info!(
        search_title = %search_title,
        episode = %args.episode,
        select_index = select_index,
        mode = %args.mode,
        quality = quality,
        "play: spawning ani-cli",
    );

    let resolved = run_debug_streaming(
        &opts,
        &search_title,
        &args.episode,
        quality,
        &args.mode,
        select_index,
        |line| {
            // Mirror every ani-cli stderr line into our own logs so a
            // failed play has a paper trail. parse_progress_line still
            // runs on the same line for the SSE overlay.
            tracing::info!(line = %line, "anicli.stderr");
            if let Some(p) = parse_progress_line(line) {
                on_progress(p);
            }
        },
    )
    .await
    .inspect_err(|e| note_spawn_failure(state, args, &search_title, select_index, e));
    record_spawn_outcome(state, &resolved);
    let resolved = resolved?;
    enrich_availability_after_success(state, args, chosen_candidate.as_ref()).await;

    // Decide media kind: cheap path-extension first, HEAD fallback
    // when the URL is opaque (fast4speed.rsvp/<id>/sub/1, etc).
    let upstream_url =
        url::Url::parse(&resolved.selected_url).map_err(|_| AniError::ParseFailed {
            detail: format!("upstream_url: {} is not a valid URL", resolved.selected_url),
        })?;

    // Infer Referer when ani-cli's debug output didn't include one.
    // Mirrors `refr_flag` switch in ani-cli (line ~209): the
    // tools.fast4speed.rsvp CDN enforces Referer = https://allmanga.to
    // and 403s requests without it. ani-cli sets the header internally
    // when invoking the player but doesn't surface it on stdout, so
    // the parser sees None for these URLs.
    let referer = match resolved.referer {
        Some(r) if !r.is_empty() => r,
        _ => match upstream_url.host_str() {
            Some(h) if h.ends_with("fast4speed.rsvp") => "https://allmanga.to".to_string(),
            _ => String::new(),
        },
    };

    let kind = match MediaKind::from_url(&upstream_url) {
        Some(k) => k,
        None => {
            // HEAD failures fall back to MP4 — that's the safe default
            // (binary streams, unknown CDNs). The proxy then serves
            // /file.mp4 with byte-range support; if the upstream truly
            // is an HLS manifest mislabelled, hls.js never enters the
            // picture and the renderer surfaces a real error.
            upstream::classify_via_head(&state.meta_http, &upstream_url, &referer)
                .await
                .unwrap_or(MediaKind::Mp4)
        }
    };
    tracing::info!(
        title = %args.title,
        episode = %args.episode,
        upstream = upstream_url.as_str(),
        referer = referer.as_str(),
        kind = ?kind,
        "play: ani-cli resolved upstream",
    );

    // Persist the resolution so the next play of the same episode
    // skips ani-cli entirely (subject to TTL + HEAD validation).
    // show_id + show_title come from the chosen allanime candidate
    // (when our search picked one) so a future cache-hit can write to
    // ani-hsts ourselves — ani-cli's update_history doesn't fire when
    // we skip the subprocess on a cache hit.
    let (show_id, show_title) = chosen_candidate
        .as_ref()
        .map(|c| {
            (
                c.id.clone(),
                format!(
                    "{} ({} episodes)",
                    c.name,
                    c.available_episodes.for_mode(&args.mode)
                ),
            )
        })
        .unwrap_or_default();
    let cached_resolution = CachedResolution {
        upstream_url: resolved.selected_url.clone(),
        referer: referer.clone(),
        subtitle_url: resolved.subtitle_url.clone(),
        media_kind: kind,
        show_id,
        show_title,
    };
    play_resolution_cache::put(&state.cache_pool, &cache_key, &cached_resolution);

    let session_args = CreateSessionArgs {
        upstream_url: resolved.selected_url,
        referer,
        subtitle_url: resolved.subtitle_url,
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
    use crate::anicli::parser::DebugOutput;

    // `play()` and `play_external()` are thin wrappers around
    // `run_debug` + the relevant terminal action; the integration
    // test in `tests/api_play.rs` exercises the full flow against a
    // real ani-cli with a curl shim. These unit tests pin the
    // mapping from `DebugOutput` → `CreateSessionArgs` /
    // `LaunchArgs` so a future refactor of the field names is loud.

    #[test]
    fn debug_output_with_referer_and_subtitle_maps_to_session_args() {
        let debug = DebugOutput {
            selected_url: "https://wixmp.example/video.mp4".into(),
            all_links: vec![],
            referer: Some("https://allmanga.to".into()),
            subtitle_url: Some("https://wixmp.example/subs.vtt".into()),
        };
        // Mirrors the conversion inside `play()`. Kept in sync via
        // the integration test; this asserts the field-by-field
        // mapping is intact.
        let session_args = CreateSessionArgs {
            upstream_url: debug.selected_url.clone(),
            referer: debug.referer.clone().unwrap_or_default(),
            subtitle_url: debug.subtitle_url.clone(),
        };
        assert_eq!(session_args.upstream_url, "https://wixmp.example/video.mp4");
        assert_eq!(session_args.referer, "https://allmanga.to");
        assert_eq!(
            session_args.subtitle_url.as_deref(),
            Some("https://wixmp.example/subs.vtt")
        );
    }

    #[test]
    fn debug_output_without_referer_maps_to_empty_referer_string() {
        // CreateSessionArgs.referer is a required `String` (not
        // Option). We map None → empty string; the proxy treats that
        // as "send no Referer header." This test pins that contract.
        let debug = DebugOutput {
            selected_url: "https://x/y.mp4".into(),
            all_links: vec![],
            referer: None,
            subtitle_url: None,
        };
        let session_args = CreateSessionArgs {
            upstream_url: debug.selected_url,
            referer: debug.referer.unwrap_or_default(),
            subtitle_url: debug.subtitle_url,
        };
        assert_eq!(session_args.referer, "");
        assert!(session_args.subtitle_url.is_none());
    }

    /// Build an `AppState` for the `try_serve_cached` tests. Mirrors
    /// `app::tests::fake_state` (private, unreachable from here) so the
    /// shape stays in lock-step.
    fn state_with_proxy_origin() -> AppState {
        use crate::meta::kitsu::KitsuClient;
        use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
        use std::sync::Arc;
        AppState {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            meta_http: reqwest::Client::new(),
            proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
            ani_cli_path: std::path::PathBuf::from("/tmp/ani-cli"),
            bash_path: None,
            bundled_bin: None,
            history_path: std::path::PathBuf::from("/tmp/ani-cli/ani-hsts"),
            scraper_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
            image_cache_dir: std::path::PathBuf::from("/tmp/ani-gui-images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: KitsuClient::new(reqwest::Client::new()),
            config_path: std::path::PathBuf::from("/tmp/ani-gui-config.toml"),
            state_dir: std::path::PathBuf::from("/tmp/ani-gui-state"),
            internal_secret: crate::account::InternalSecret::random(),
            mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
            account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        }
    }

    /// Build a CachedResolution with the new show_id/show_title fields
    /// defaulted to empty (so try_serve_cached's history-write skip
    /// branch fires). Tests that want history-write coverage override
    /// the two fields explicitly.
    fn cached_blank(upstream_url: String, referer: String, kind: MediaKind) -> CachedResolution {
        CachedResolution {
            upstream_url,
            referer,
            subtitle_url: None,
            media_kind: kind,
            show_id: String::new(),
            show_title: String::new(),
        }
    }

    #[tokio::test]
    async fn try_serve_cached_returns_none_when_url_is_unparseable() {
        // A corrupt cache row with garbage in upstream_url shouldn't
        // crash — fall through to ani-cli.
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
        // ~50ms path that replaces the ~30s ani-cli spawn.
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
        // must set it; the post-ani-cli path must not.
        assert!(
            resp.cache_hit,
            "try_serve_cached must tag the response so the renderer can retry on player error"
        );
    }

    #[tokio::test]
    async fn try_serve_cached_returns_none_on_404() {
        // Stale wixmp URL — HEAD 404 means the row is dead. Return
        // None so the caller falls through to ani-cli (which will
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
        );
        play_resolution_cache::put(
            &state.cache_pool,
            &key,
            &CachedResolution {
                upstream_url: upstream.into(),
                referer: referer.into(),
                subtitle_url: None,
                media_kind: MediaKind::Mp4,
                show_id: "abc".into(),
                show_title: "Test (12 episodes)".into(),
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
        // target is state.history_path, which is /tmp/ani-cli/ani-hsts
        // by default — make it a real tempfile so the write
        // succeeds and we can assert against it.
        let td = tempfile::tempdir().expect("tempdir");
        let mut state = state;
        state.history_path = td.path().join("ani-hsts");
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
    /// ani-cli (which fails because the spawn binary path is bogus
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
        assert!(r.is_err(), "ani-cli fallback must error in the test env");
        // Cache row should be gone.
        let key = play_resolution_cache::cache_key(
            &args.title,
            &args.mode,
            "best",
            &args.episode,
            args.year,
            args.episode_count,
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
        // returned LaunchArgs to mpv without re-running ani-cli.
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
        // fresh ani-cli run will overwrite, AND we return None so the
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
    async fn try_launch_args_from_cache_round_trips_referer_and_subtitle() {
        // fast4speed.rsvp + signed-URL upstreams need the cached
        // Referer header forwarded; subtitle URL too (mpv consumes it).
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
        );
        play_resolution_cache::put(
            &state.cache_pool,
            &key,
            &CachedResolution {
                upstream_url: format!("{}/sub/3", server.uri()),
                referer: "https://allmanga.to".into(),
                subtitle_url: Some("https://example/cap.vtt".into()),
                media_kind: MediaKind::Mp4,
                show_id: "x".into(),
                show_title: "Fast4 (12 episodes)".into(),
            },
        );
        let cfg = external_cfg();

        let launch = try_launch_args_from_cache(&state, &args, &cfg)
            .await
            .expect("hit");
        assert_eq!(launch.referer.as_deref(), Some("https://allmanga.to"));
        assert_eq!(
            launch.subtitle_url.as_deref(),
            Some("https://example/cap.vtt")
        );
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

    /// Build a Candidate row with the right `availableEpisodes.sub`
    /// field for the helper-selection tests below. The full struct
    /// is verbose; this keeps each test focused on the behaviour it's
    /// asserting (which title wins, which candidate index ani-cli
    /// gets).
    fn cand(id: &str, name: &str, sub_eps: u32) -> Candidate {
        Candidate {
            id: id.into(),
            name: name.into(),
            available_episodes: crate::scraper::allanime::AvailableEpisodes {
                sub: sub_eps,
                dub: 0,
            },
            ..Default::default()
        }
    }

    #[test]
    fn select_first_with_hits_walks_alt_titles_when_episode_count_unknown() {
        // Real-world reproducer: Kitsu returns null `episodeCount` for
        // some shows even when they're finished (Stone Ocean Part 6 was
        // observed in the wild). The early `let Some(expected) = …`
        // guard used to short-circuit the alt_titles loop, which meant
        // `args.title` was the only thing tried — and for shows whose
        // canonical doesn't match allmanga's index, that's a guaranteed
        // miss. The fix: always walk the candidate list, only the
        // pick_by_ep_count step is gated on episode_count.
        let results = vec![
            ("JoJo's Bizarre Adventure: Stone Ocean".into(), vec![]),
            (
                "Jojo no Kimyou na Bouken Part 6: Stone Ocean".into(),
                vec![cand("a1", "Stone Ocean", 12)],
            ),
        ];
        // expected = None signals "no Kitsu episode_count to disambiguate".
        let (title, idx) = select_first_with_hits_opt(
            "JoJo's Bizarre Adventure: Stone Ocean",
            &results,
            None,
            None,
            "sub",
        );
        assert_eq!(title, "Jojo no Kimyou na Bouken Part 6: Stone Ocean");
        assert_eq!(idx, 1, "first hit when no ep_count to compare");
    }

    #[test]
    fn select_first_with_hits_returns_primary_when_every_list_is_empty() {
        // Stone Ocean reproduces this when every candidate title (canonical
        // English + en_jp + ja_jp) misses allmanga's index. We fall
        // through to the primary so the play flow's downstream error
        // surfaces a real "no upstream" rather than silently picking
        // index 1 of nothing.
        let results: Vec<(String, Vec<Candidate>)> =
            vec![("primary".into(), vec![]), ("alt1".into(), vec![])];
        let (title, idx) = select_first_with_hits("primary", &results, 38, "sub");
        assert_eq!(title, "primary");
        assert_eq!(idx, 1);
    }

    #[test]
    fn select_first_with_hits_uses_first_non_empty_list() {
        // Primary has hits — we never even look at the alt titles.
        let results = vec![
            ("primary".into(), vec![cand("p1", "Primary Show", 38)]),
            ("alt1".into(), vec![cand("a1", "Alt Show", 12)]),
        ];
        let (title, idx) = select_first_with_hits("primary", &results, 38, "sub");
        assert_eq!(title, "primary");
        assert_eq!(idx, 1, "single-candidate list always picks index 1");
    }

    #[test]
    fn select_first_with_hits_skips_empty_primary_to_alt_with_hits() {
        // Stone Ocean Part 6 case: canonical English → 0 hits, en_jp
        // → multiple hits. We must use en_jp.
        let results = vec![
            ("JoJo's Bizarre Adventure: Stone Ocean".into(), vec![]),
            (
                "Jojo no Kimyou na Bouken Part 6: Stone Ocean".into(),
                vec![
                    cand("a1", "Stone Ocean main", 38),
                    cand("a2", "side story", 1),
                ],
            ),
        ];
        let (title, idx) =
            select_first_with_hits("JoJo's Bizarre Adventure: Stone Ocean", &results, 38, "sub");
        assert_eq!(title, "Jojo no Kimyou na Bouken Part 6: Stone Ocean");
        // 38-ep candidate is index 1 (closer to expected 38 than the
        // 1-ep side story).
        assert_eq!(idx, 1);
    }

    fn picked_miss(any_success: bool, any_error: bool) -> PickedTitle {
        PickedTitle {
            title: "X".into(),
            index: 0,
            candidate: None,
            any_search_succeeded: any_success,
            any_search_errored: any_error,
        }
    }

    #[test]
    fn picker_miss_caller_error_treats_partial_failure_as_network() {
        // Mixed: at least one search succeeded but another errored.
        // Sibling callers (download/syncplay/external) prefer the
        // transient surface — the failed search may have been the
        // one with the right match. Codex P2 #3235184271.
        let err = picker_miss_caller_error(&picked_miss(true, true));
        assert!(matches!(err, AniError::Network));
    }

    #[test]
    fn picker_miss_caller_error_returns_no_results_only_when_all_searches_completed() {
        let err = picker_miss_caller_error(&picked_miss(true, false));
        assert!(matches!(err, AniError::NoResults));
    }

    #[test]
    fn picker_miss_caller_error_treats_all_errored_as_network() {
        let err = picker_miss_caller_error(&picked_miss(false, true));
        assert!(matches!(err, AniError::Network));
    }

    #[test]
    fn classify_picker_miss_returns_network_on_partial_search_failure() {
        // Codex P2 #3236... : the embedded play path used to surface
        // NoResults when one search errored alongside a success that
        // produced no safe match. The verdict is incomplete in that
        // case — the failed search may have been the one with the
        // canonical hit — so the embedded surface should agree with
        // download / play_external / play_syncplay / availability and
        // return transient Network instead.
        let state = state_with_proxy_origin();
        let args = PlayArgs {
            title: "X".into(),
            episode: "1".into(),
            mode: "sub".into(),
            quality: None,
            episode_count: None,
            year: None,
            alt_titles: vec![],
            prefetch: false,
            kitsu_id: None,
        };
        let err = classify_picker_miss(&state, &args, &picked_miss(true, true));
        assert!(
            matches!(err, AniError::Network),
            "partial failure should surface Network, got {err:?}",
        );
    }

    #[test]
    fn select_first_with_hits_picks_by_ep_count_within_chosen_list() {
        // Naruto: Shippuden case — multiple candidates under one title;
        // the disambiguator chooses by episode count.
        let results = vec![(
            "Naruto: Shippuden".into(),
            vec![
                cand("a1", "side story", 1),
                cand("a2", "main shippuden", 500),
            ],
        )];
        let (title, idx) = select_first_with_hits("Naruto: Shippuden", &results, 500, "sub");
        assert_eq!(title, "Naruto: Shippuden");
        // Index 2 = the 500-ep main show.
        assert_eq!(idx, 2);
    }

    /// PlayArgs shaped like a background prefetch / interactive click
    /// for the scraper-gate tests.
    fn gate_args(prefetch: bool, title: &str, alts: &[&str]) -> PlayArgs {
        PlayArgs {
            title: title.into(),
            episode: "1".into(),
            mode: "sub".into(),
            quality: None,
            episode_count: None,
            year: None,
            alt_titles: alts.iter().map(|s| (*s).to_string()).collect(),
            prefetch,
            kitsu_id: None,
        }
    }

    #[tokio::test]
    async fn background_pick_stops_probing_after_the_breaker_opens() {
        // A cold-cache warm walked every alt title of every show even
        // while allanime was refusing us — hundreds of doomed calls
        // that deepened the rate limit. Once the gate's breaker opens
        // the walk must stop.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        let args = gate_args(true, "Gate Test", &["a", "b", "c", "d", "e"]);
        let picked = pick_title_and_index_with_base(&state, &args, Some(&server.uri())).await;
        assert!(picked.candidate.is_none());
        assert!(picked.any_search_errored);
        let hits = server.received_requests().await.expect("recorded").len();
        assert_eq!(
            hits,
            crate::scraper::gate::FAILURE_THRESHOLD as usize,
            "the alt-title walk must stop at the breaker threshold, not visit all six titles"
        );
    }

    #[tokio::test]
    async fn prefetch_spawn_admission_respects_an_open_breaker() {
        // The subprocess makes its own allanime requests; a prefetch
        // spawn is background traffic and must be refused while the
        // breaker is open. Interactive plays pass untouched.
        let state = state_with_proxy_origin();
        for _ in 0..crate::scraper::gate::FAILURE_THRESHOLD {
            state.scraper_gate.record_outcome(false);
        }
        let prefetch = gate_args(true, "Gate Test", &[]);
        assert!(matches!(
            admit_prefetch_spawn(&state, &prefetch).await,
            Err(AniError::Network)
        ));
        let interactive = gate_args(false, "Gate Test", &[]);
        assert!(admit_prefetch_spawn(&state, &interactive).await.is_ok());
    }

    #[tokio::test]
    async fn prefetch_spawn_no_results_counts_toward_the_breaker() {
        use crate::scraper::gate::ScrapePriority;
        // Transport-ish spawn failures count toward the breaker...
        let state = state_with_proxy_origin();
        for _ in 0..crate::scraper::gate::FAILURE_THRESHOLD {
            record_spawn_outcome::<()>(&state, &Err(AniError::Io));
        }
        assert!(
            state
                .scraper_gate
                .admit(ScrapePriority::Background)
                .await
                .is_err(),
            "repeated spawn failures must open the breaker"
        );
        // ...and so does NoResults: a spawn only happens after the
        // picker just confirmed the show exists on allanime, so the
        // subprocess finding nothing moments later is transient or
        // upstream evidence (a rate-limited ani-cli dies with exactly
        // this message), not absence.
        let state = state_with_proxy_origin();
        for _ in 0..crate::scraper::gate::FAILURE_THRESHOLD {
            record_spawn_outcome::<()>(&state, &Err(AniError::NoResults));
        }
        assert!(
            state
                .scraper_gate
                .admit(ScrapePriority::Background)
                .await
                .is_err(),
            "repeated NoResults spawns must open the breaker"
        );
    }

    #[tokio::test]
    async fn interactive_spawn_failures_feed_the_breaker_too() {
        use crate::scraper::gate::ScrapePriority;
        // A user click bypasses admission, but its subprocess outcome
        // still matters: the preflight search just reset the breaker
        // with a success, so if the spawn is what hits the rate limit,
        // background traffic must back off — not resume immediately
        // after a user-visible failure.
        let state = state_with_proxy_origin();
        for _ in 0..crate::scraper::gate::FAILURE_THRESHOLD {
            record_spawn_outcome::<()>(&state, &Err(AniError::NoResults));
        }
        assert!(state
            .scraper_gate
            .admit(ScrapePriority::Background)
            .await
            .is_err());
    }

    #[test]
    fn no_results_spawn_failure_does_not_write_a_negative_availability_row() {
        // The gate treats a spawn-level NoResults as transient — the
        // picker confirmed the show exists moments earlier — so
        // persisting available=false from the same signal would hide
        // a real show behind the negative TTL. Genuine absence is
        // recorded by the picker path (classify_picker_miss), which
        // reaches its verdict from clean zero-candidate searches.
        let state = state_with_proxy_origin();
        let mut args = gate_args(false, "Gate Test", &[]);
        args.kitsu_id = Some("777".into());
        note_spawn_failure(&state, &args, "Gate Test", 1, &AniError::NoResults);
        let cached = crate::commands::availability::batch_cached(
            &state,
            &crate::commands::availability::AvailabilityBatchArgs {
                kitsu_ids: vec!["777".into()],
                mode: "sub".into(),
            },
        );
        assert!(
            cached.cached.is_empty(),
            "transient spawn miss must not poison the availability cache"
        );
    }

    #[tokio::test]
    async fn prefetch_spawn_scraper_verdicts_leave_the_breaker_alone() {
        use crate::scraper::gate::ScrapePriority;
        // Scraper{} carries content-level verdicts — an ep+1 prefetch
        // at the season edge dies with "Episode not released", which
        // is a correct answer, not network trouble. Counting it would
        // open the breaker from ordinary prefetching.
        let state = state_with_proxy_origin();
        for _ in 0..crate::scraper::gate::FAILURE_THRESHOLD + 2 {
            record_spawn_outcome::<()>(
                &state,
                &Err(AniError::Scraper {
                    key: crate::i18n::keys::SCRAPER_PARSE_FAILED,
                }),
            );
        }
        assert!(state
            .scraper_gate
            .admit(ScrapePriority::Background)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn interactive_pick_bypasses_an_open_breaker() {
        // Guard for the other direction: a user's click goes through
        // even when background traffic tripped the breaker.
        let server = wiremock::MockServer::start().await;
        let body = serde_json::json!({
            "data": { "shows": { "edges": [{
                "_id": "abc",
                "name": "Gate Test",
                "availableEpisodes": {"sub": 12, "dub": 0, "raw": 0},
                "__typename": "Show"
            }]}}
        });
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let state = state_with_proxy_origin();
        for _ in 0..crate::scraper::gate::FAILURE_THRESHOLD {
            state.scraper_gate.record_outcome(false);
        }
        let args = gate_args(false, "Gate Test", &[]);
        let picked = pick_title_and_index_with_base(&state, &args, Some(&server.uri())).await;
        assert!(picked.any_search_succeeded);
        assert!(picked.candidate.is_some());
    }
}
