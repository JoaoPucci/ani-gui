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
use crate::scraper::gate::{ScrapePriority, ScraperGate};

use super::play_native::{pick_candidate, PickedShow};
use super::play_native_numbering::{kitsu_episode_cap, numbering_offset};

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

/// Search the request's title then its fallbacks in order, pick, and
/// resolve the episode for the mode to a master-playlist URL.
///
/// - Each provider request cluster is admitted through `gate` at
///   `priority` when a gate is given; a refused admit is transient.
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
    gate: Option<&ScraperGate>,
    priority: ScrapePriority,
    req: NativeResolveRequest<'_>,
    on_progress: &mut P,
) -> std::result::Result<NativeResolved, NativeError>
where
    F: AnidbFetch,
    P: FnMut(ProgressLine) + Send,
{
    on_progress(ProgressLine::Banner {
        text: "Searching anidb.app...".into(),
    });
    let mut any_search_succeeded = false;
    let mut any_search_errored = false;
    for t in std::iter::once(req.title).chain(req.alt_titles.iter().map(String::as_str)) {
        if let Some(g) = gate {
            if g.admit(priority).await.is_err() {
                return Err(NativeError {
                    error: AniError::Network,
                    clean_miss: false,
                });
            }
        }
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
                        on_progress(ProgressLine::Other {
                            text: format!("Matched {}", picked.hit.title),
                        });
                        let master_url =
                            resolve_episode(client, &picked, req.episode, req.mode, req.quality)
                                .await?;
                        on_progress(ProgressLine::LinksFetched {
                            provider: "anidb.app".into(),
                        });
                        let episode_cap = kitsu_episode_cap(&picked.episodes);
                        let offset = numbering_offset(&picked.episodes);
                        return Ok(NativeResolved {
                            slug: picked.hit.slug,
                            title: picked.hit.title,
                            master_url,
                            episode_cap,
                            numbering_offset: offset,
                        });
                    }
                    // A rejected pool is a clean verdict about THIS
                    // pool; the next alias may carry the real show.
                    Err(AniError::NoResults) => {}
                    // A refusal or rate limit from a probe is the
                    // provider blocking this client: the walk stops,
                    // as it does when the search itself is refused.
                    Err(e) if e.is_provider_block() => {
                        return Err(NativeError {
                            error: e,
                            clean_miss: false,
                        });
                    }
                    Err(_) => {
                        any_search_errored = true;
                    }
                }
            }
            Err(AniError::Upstream { status }) => {
                // Blocked: every further request deepens the hole.
                return Err(NativeError {
                    error: AniError::Upstream { status },
                    clean_miss: false,
                });
            }
            Err(_) => {
                any_search_errored = true;
            }
        }
    }
    if !any_search_succeeded || any_search_errored {
        return Err(NativeError {
            error: AniError::Network,
            clean_miss: false,
        });
    }
    Err(NativeError {
        error: AniError::NoResults,
        clean_miss: true,
    })
}

/// Resolve the requested episode within a picked show down to the
/// master URL. Split from the walk for the per-file complexity bar
/// and because the orchestrator's cache-hit path may someday reuse
/// it. The request's number is per-entry; the provider's listing may
/// be cumulative — [`numbering_offset`] bridges the two.
///
/// # Errors
/// `NativeError` (never `clean_miss`): the show matched, so nothing
/// here is evidence of absence.
pub async fn resolve_episode<F: AnidbFetch>(
    client: &AnidbClient<F>,
    picked: &PickedShow,
    episode: &str,
    mode: &str,
    quality: &str,
) -> std::result::Result<String, NativeError> {
    let dead_end = |error: AniError| NativeError {
        error,
        clean_miss: false,
    };
    let n: u32 = episode
        .trim()
        .parse()
        .map_err(|_| dead_end(AniError::NoResults))?;
    let n = n.saturating_add(numbering_offset(&picked.episodes));
    let ep = picked
        .episodes
        .iter()
        .find(|e| e.number == n)
        .ok_or_else(|| dead_end(AniError::NoResults))?;
    let master = client
        .master_playlist_url(ep.id, mode)
        .await
        .map_err(dead_end)?;
    // The quality step is soft: any miss keeps the adaptive master.
    Ok(client.quality_stream_url(&master, quality).await)
}

#[cfg(test)]
#[path = "play_native_resolve_test.rs"]
mod tests;
