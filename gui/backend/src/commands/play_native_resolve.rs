//! Native play resolution — the walk half. Searches the provider
//! across the canonical title and its fallbacks, picks a candidate,
//! and resolves the requested episode down to a master-playlist URL,
//! emitting the same [`ProgressLine`] shapes the SSE overlay already
//! renders.
//!
//! Error policy mirrors what the subprocess path converged on: a
//! clean everywhere-searched-nothing-matched miss is the only verdict
//! the caller may persist as a negative availability write
//! ([`NativeError::clean_miss`]); every transport failure, upstream
//! refusal, or post-pick dead end is transient and must not hide a
//! real show behind the negative TTL. An upstream refusal
//! (cloudflare) stops the walk — a block on one query blocks them
//! all, and each further request deepens the hole.

use crate::anicli::parser::ProgressLine;
use crate::error::AniError;
use crate::scraper::anidb::{AnidbClient, AnidbFetch};

use super::play_native::pick_candidate;
pub use super::play_native_episode::resolve_episode;
use super::play_native_episode::{classify_chain_failure, ChainOutcome};
use super::play_native_numbering::{extra_episode_tags, kitsu_episode_cap, numbering_offset};

/// A fully resolved native play: what the orchestrator needs to open
/// a session, stamp caches, and write history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeResolved {
    /// The provider slug — the id history and caches key on.
    pub slug: String,
    /// The provider's display title for the show.
    pub title: String,
    /// The master-playlist URL the embed page carried.
    pub master_url: String,
    /// Highest episode number the provider lists, for the
    /// availability cap stamp. Free — the picker already fetched the
    /// list.
    pub episode_cap: Option<u32>,
    /// The shift between the provider's episode numbers and the
    /// per-entry numbers the request used ([`numbering_offset`]).
    /// The ani-hsts writers add it — the shared history speaks the
    /// provider's numbering — and the offset store persists it for
    /// the cache-hit and mark-watched writers and the read boundary.
    pub numbering_offset: u32,
    /// The listing's fractional display tags
    /// ([`extra_episode_tags`]) — the availability stamp's
    /// `extra_episodes`, also already paid for.
    pub extra_tags: Vec<String>,
    /// The matched row's slot — the integer the CLI's history
    /// reader greps, and what the fresh-resolve writer stores.
    pub resolved_slot: u32,
    /// The matched row's display tag, stamped beside the offset when
    /// it differs from the slot so the read boundary can translate.
    pub resolved_tag: Option<String>,
}

/// A failed resolution, carrying the typed error plus whether the
/// verdict is a clean picker miss the caller may persist.
#[derive(Debug)]
pub struct NativeError {
    /// The error to surface.
    pub error: AniError,
    /// True only when every search completed cleanly and no candidate
    /// survived — the one shape that proves absence rather than
    /// weather.
    pub clean_miss: bool,
    /// The instant the failure behind an AGGREGATED transient
    /// verdict was observed. The walk can fail one alias and finish
    /// a later one cleanly; the breaker record for the resulting
    /// `Network` must carry the failing attempt's moment, or the
    /// gate's staleness filter cannot discard it after a recovery
    /// recorded in between. `None` when the error IS the chain's
    /// last attempt — the transport's own stamp is then correct.
    pub failed_at: Option<tokio::time::Instant>,
}

/// What a native resolution is asked for — the play-relevant slice
/// of `PlayArgs`, borrowed.
#[derive(Debug, Clone, Copy)]
pub struct NativeResolveRequest<'a> {
    /// Canonical title, searched first.
    pub title: &'a str,
    /// Fallback titles, searched in order.
    pub alt_titles: &'a [String],
    /// Episode number as the frontend sent it.
    pub episode: &'a str,
    /// `"sub"` or `"dub"`.
    pub mode: &'a str,
    /// Quality setting (`best`, `worst`, or a height like `720`).
    /// `best` keeps the adaptive master; anything else selects a
    /// variant from it, falling back to the master on a miss.
    pub quality: &'a str,
    /// Kitsu's episode count, when known.
    pub expected_count: Option<u32>,
    /// The year the show premiered per Kitsu, when known. The
    /// picker's identity signal for cour and franchise siblings.
    pub year: Option<u32>,
    /// Kitsu's subtype (`TV`, `movie`, `special`, `OVA`, `ONA`),
    /// when the caller has it. Format disproof against the browse
    /// cards' badges.
    pub subtype: Option<&'a str>,
}

/// Ceiling on one complete native resolution — every search, probe
/// and embed fetch together. Strictly below
/// [`crate::scraper::gate::HALF_OPEN_TRIAL_STALE`]: a resolve that
/// outlived the trial window would let a second half-open trial
/// start behind the still-running first, the overlap the sanction
/// chain forbids. 60s also matches the ceiling the subprocess path
/// enforced with its run timeout.
pub const RESOLVE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(60);

/// [`resolve_native`] under [`RESOLVE_DEADLINE`]. Elapsing is the
/// stall's own identity: [`AniError::Timeout`], never a clean miss —
/// a provider that accepts connections but answers nothing is
/// weather, and the breaker hears it as such.
///
/// # Errors
/// As [`resolve_native`], plus `Timeout` at the deadline.
pub async fn resolve_native_bounded<F, P>(
    client: &AnidbClient<F>,
    req: NativeResolveRequest<'_>,
    on_progress: &mut P,
) -> std::result::Result<NativeResolved, NativeError>
where
    F: AnidbFetch,
    P: FnMut(ProgressLine) + Send,
{
    match tokio::time::timeout(RESOLVE_DEADLINE, resolve_native(client, req, on_progress)).await {
        Ok(resolved) => resolved,
        Err(_elapsed) => Err(NativeError {
            error: AniError::Timeout,
            clean_miss: false,
            failed_at: None,
        }),
    }
}

/// Search the request's title then its fallbacks in order, pick, and
/// resolve the episode for the mode to a master-playlist URL.
///
/// - Gate admission is the transport's job: the production client
///   wraps its fetch in [`crate::scraper::anidb::GatedFetch`], which
///   admits every provider request and carries the half-open trial
///   sanction across the chain. A second admission here would consume
///   that trial before the fetch's own admit runs — the fetch would
///   then be refused by its own chain's sanction, and the breaker
///   would reopen on its own refusal.
/// - A pool whose pick is rejected keeps the walk going — the next
///   alias may carry the real show (the Stone Ocean recovery).
/// - [`AniError::Upstream`] from a search stops the walk.
/// - `episode` must name an episode the provider lists; anything else
///   is a transient dead end, not evidence of absence.
///
/// # Errors
/// [`NativeError`] with `clean_miss` set only for the
/// all-clean-no-match verdict.
pub async fn resolve_native<F, P>(
    client: &AnidbClient<F>,
    req: NativeResolveRequest<'_>,
    on_progress: &mut P,
) -> std::result::Result<NativeResolved, NativeError>
where
    F: AnidbFetch,
    P: FnMut(ProgressLine) + Send,
{
    on_progress(ProgressLine::Searching {
        provider: "anidb.app".into(),
    });
    let mut any_search_succeeded = false;
    let mut any_search_errored = false;
    let mut any_answered_dead_end = false;
    let mut last_failure_at: Option<tokio::time::Instant> = None;
    for t in std::iter::once(req.title).chain(req.alt_titles.iter().map(String::as_str)) {
        match client.search(t).await {
            Ok(hits) => {
                any_search_succeeded = true;
                if hits.is_empty() {
                    continue;
                }
                match pick_candidate(client, &hits, req.expected_count, t, req.year, req.subtype)
                    .await
                {
                    Ok(picked) => {
                        on_progress(ProgressLine::Matched {
                            title: picked.hit.title.clone(),
                        });
                        let resolved = match resolve_episode(
                            client,
                            &picked,
                            req.episode,
                            req.mode,
                            req.quality,
                        )
                        .await
                        {
                            Ok(resolved) => resolved,
                            Err(ne) => match classify_chain_failure(ne) {
                                ChainOutcome::Stop(ne) => return Err(ne),
                                ChainOutcome::DeadEnd => {
                                    any_answered_dead_end = true;
                                    continue;
                                }
                                ChainOutcome::Transient => {
                                    any_search_errored = true;
                                    last_failure_at = Some(
                                        client
                                            .transport()
                                            .last_attempt_at()
                                            .unwrap_or_else(tokio::time::Instant::now),
                                    );
                                    continue;
                                }
                            },
                        };
                        on_progress(ProgressLine::LinksFetched {
                            provider: "anidb.app".into(),
                        });
                        let episode_cap = kitsu_episode_cap(&picked.episodes);
                        let offset = numbering_offset(&picked.episodes);
                        let extra_tags = extra_episode_tags(&picked.episodes);
                        return Ok(NativeResolved {
                            slug: picked.hit.slug,
                            title: picked.hit.title,
                            master_url: resolved.master_url,
                            episode_cap,
                            numbering_offset: offset,
                            extra_tags,
                            resolved_slot: resolved.slot,
                            resolved_tag: resolved.tag,
                        });
                    }
                    // A rejected pool is a clean verdict about THIS
                    // pool; the next alias may carry the real show.
                    Err(AniError::NoResults) => {}
                    // A refusal or rate limit from a probe is the
                    // provider blocking this client — and a gate
                    // refusal is the gate's own stop — either way
                    // the walk ends, identity intact.
                    Err(e) if e.is_provider_block() || matches!(e, AniError::GateRefused) => {
                        return Err(NativeError {
                            error: e,
                            clean_miss: false,
                            failed_at: None,
                        });
                    }
                    // An all-answered-not-found pool: dead candidates
                    // on a healthy provider. The walk moves on, and
                    // the verdict stays answered — the breaker must
                    // not open on a provider that answered.
                    Err(AniError::Upstream { .. }) => {
                        any_answered_dead_end = true;
                    }
                    Err(_) => {
                        any_search_errored = true;
                        last_failure_at = Some(
                            client
                                .transport()
                                .last_attempt_at()
                                .unwrap_or_else(tokio::time::Instant::now),
                        );
                    }
                }
            }
            Err(AniError::Upstream { status }) => {
                // Blocked: every further request deepens the hole.
                return Err(NativeError {
                    error: AniError::Upstream { status },
                    clean_miss: false,
                    failed_at: None,
                });
            }
            // The gate refused this chain: every subsequent fetch
            // would be refused the same way. The identity survives
            // to the caller, which records no breaker outcome for it.
            Err(AniError::GateRefused) => {
                return Err(NativeError {
                    error: AniError::GateRefused,
                    clean_miss: false,
                    failed_at: None,
                });
            }
            Err(_) => {
                any_search_errored = true;
                last_failure_at = Some(
                    client
                        .transport()
                        .last_attempt_at()
                        .unwrap_or_else(tokio::time::Instant::now),
                );
            }
        }
    }
    if !any_search_succeeded || any_search_errored {
        return Err(NativeError {
            error: AniError::Network,
            clean_miss: false,
            failed_at: last_failure_at,
        });
    }
    if any_answered_dead_end {
        // Answered dead ends prove nothing about the show — never
        // the persistable clean miss — but the provider answering is
        // health to the breaker (NoResults records success).
        return Err(NativeError {
            error: AniError::NoResults,
            clean_miss: false,
            failed_at: None,
        });
    }
    Err(NativeError {
        error: AniError::NoResults,
        clean_miss: true,
        failed_at: None,
    })
}

#[cfg(test)]
#[path = "play_native_resolve_test.rs"]
mod tests;
