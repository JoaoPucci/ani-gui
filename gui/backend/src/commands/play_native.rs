//! Native play resolution against the anidb provider — the picker
//! half. Replaces the "compute a `-S` index and hope the script's
//! search returns the same list" coupling with a direct pick over the
//! client's own results.
//!
//! The browse page carries titles only, so episode counts come from
//! one episodes call per considered candidate. The probe set is
//! bounded: real queries put the right show in the first few hits,
//! and every probe is an upstream request. Within the probed set the
//! proven layers from the allanime picker apply — episode-count
//! distance with the `max(3, 10%)` threshold, then an exact-name
//! tie-break — minus the year filter, which waits until a live
//! capture confirms where the browse markup carries a year.

use crate::error::Result;
use crate::scraper::anidb::{AnidbClient, AnidbFetch, BrowseHit};

/// How many browse hits get an episodes probe. Beyond this the match
/// was not a match; the request budget is better spent on the next
/// alias.
pub const MAX_PROBED_CANDIDATES: usize = 5;

/// A picked show: the hit plus the episode list the probe already
/// paid for, so the caller never re-fetches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedShow {
    /// The winning browse hit.
    pub hit: BrowseHit,
    /// The show's episodes, as returned by the probe.
    pub episodes: Vec<crate::scraper::anidb::EpisodeRef>,
}

/// Distance tolerance: long-running shows get proportional slack,
/// short shows a hard floor of 3 — the same rule the allanime picker
/// converged on after the sibling-mispick rounds.
pub fn ep_count_threshold(expected: u32) -> u32 {
    (expected / 10).max(3)
}

/// The considered head of `hits`, narrowed by Kitsu's year when it
/// is known: each candidate's detail page names its premiere year,
/// and a known year more than one off Kitsu's excludes the
/// candidate — the identity signal applies even to a lone candidate.
/// Only an exact year counts as positive identity; one year off is
/// tolerated (December premieres straddle catalogue years) without
/// vouching for the candidate.
/// Unknown years (a 404ing detail page, a page without a season
/// link) never exclude, so a provider markup change degrades to
/// year-blind picking rather than emptying pools. A pool whose every
/// candidate carries a known mismatched year contains the show
/// nowhere — that is a rejection ([`AniError::NoResults`]), so the
/// walk can try the next alias. Each survivor carries whether its
/// own year positively matched.
async fn year_filtered<'a, F: AnidbFetch>(
    client: &AnidbClient<F>,
    hits: &'a [BrowseHit],
    year: Option<u32>,
) -> Result<(Vec<(&'a BrowseHit, bool)>, bool)> {
    let head = hits.iter().take(MAX_PROBED_CANDIDATES);
    let Some(year) = year else {
        return Ok((head.map(|h| (h, false)).collect(), false));
    };
    let mut kept = Vec::new();
    let mut excluded_any = false;
    for h in head {
        match client.detail_year(&h.slug).await {
            // Exact match is positive identity; one year off is the
            // December-premiere allowance — enough to stay in the
            // pool and compete on count, never enough to vouch for
            // a candidate (the rescue and the countless preference
            // key on the flag, and a boundary-tolerated movie must
            // not ride them past a large count mismatch).
            Some(y) if y == year => kept.push((h, true)),
            Some(y) if y.abs_diff(year) <= 1 => kept.push((h, false)),
            Some(_) => excluded_any = true,
            None => kept.push((h, false)),
        }
    }
    if kept.is_empty() {
        return Err(crate::error::AniError::NoResults);
    }
    Ok((kept, excluded_any))
}

/// Pick the show a query meant from browse `hits`, using Kitsu's
/// `expected` episode count and premiere `year` when known.
///
/// - Considers at most [`MAX_PROBED_CANDIDATES`] hits.
/// - With `year = Some(y)`: candidates whose detail page names a
///   premiere year more than one off `y` are excluded before any
///   scoring — the identity signal that separates cour and
///   franchise siblings whose counts tie. Unknown years pass; a
///   pool whose known years all disagree is rejected outright —
///   the show is positively not in it, and the next alias may
///   carry it.
/// - With `expected = Some(n)`: best episode-count distance wins,
///   rejected when the best distance exceeds [`ep_count_threshold`];
///   ties prefer an exact (case-insensitive, trimmed) title match on
///   `search_title`. When no survivor sits within the threshold, a
///   survivor whose own year positively matched and whose episode
///   list is shorter than expected still wins — an airing part has
///   aired fewer episodes than the total Kitsu knows is coming.
/// - With `expected = None`: an exact title match wins, else the
///   first surviving hit — positional order is the provider's own
///   ranking.
/// - Probe errors skip the candidate rather than abort the pick; a
///   pick only fails when no probed candidate survives.
///
/// # Errors
/// [`crate::error::AniError::NoResults`] when `hits` is empty or no
/// candidate survives the threshold.
pub async fn pick_candidate<F: AnidbFetch>(
    client: &AnidbClient<F>,
    hits: &[BrowseHit],
    expected: Option<u32>,
    search_title: &str,
    year: Option<u32>,
    subtype: Option<&str>,
) -> Result<PickedShow> {
    if hits.is_empty() {
        // Nothing to probe: a clean absence of candidates, distinct
        // from probes that failed below.
        return Err(crate::error::AniError::NoResults);
    }
    let needle = search_title.trim().to_lowercase();
    let (head, year_excluded_any) = year_filtered(client, hits, year).await?;

    // Format disproof, the layer the allanime picker carried as its
    // type filter: a card badged `Movie` cannot be the multi-episode
    // series the caller expects — nor the clicked entry when Kitsu
    // says it is anything but a movie (the special-vs-movie shape:
    // both are single videos, count-tied, and only the format tells
    // them apart). Unknown badges never exclude, an absent subtype
    // keeps single-video pools permissive, and a pool left empty
    // here is disproven — the next alias may carry the real show.
    let expects_non_movie = matches!(expected, Some(n) if n > 1)
        || subtype.is_some_and(|s| !s.eq_ignore_ascii_case("movie"));
    let head: Vec<_> = if expects_non_movie {
        head.into_iter()
            .filter(|(h, _)| h.kind.as_deref() != Some("Movie"))
            .collect()
    } else {
        head
    };
    if head.is_empty() {
        return Err(crate::error::AniError::NoResults);
    }

    let Some(expected) = expected else {
        // No count signal: an exact title beats positional order,
        // then a candidate whose own year matched Kitsu's beats the
        // rest. When the year disproved part of the pool and no
        // survivor carries positive identity evidence, the pool is
        // token-search garbage — reject it so the next alias gets
        // its chance (the live Tai-Ari mispick: three decades-off
        // hits excluded, an unknown-year movie left standing). A
        // pool the year disproved nothing about keeps the
        // provider's own ranking, so pages without season links
        // stay resolvable. The single probe still runs so the
        // caller gets the episode list it needs.
        let exact = head
            .iter()
            .map(|(h, _)| *h)
            .find(|h| h.title.trim().to_lowercase() == needle);
        let confirmed = head.iter().find(|(_, c)| *c).map(|(h, _)| *h);
        let chosen = match (exact, confirmed) {
            (Some(h), _) => h,
            (None, Some(h)) => h,
            (None, None) if year_excluded_any => {
                return Err(crate::error::AniError::NoResults);
            }
            (None, None) => head
                .first()
                .map(|(h, _)| *h)
                .ok_or(crate::error::AniError::NoResults)?,
        };
        let episodes = client.episodes(&chosen.slug).await?;
        return Ok(PickedShow {
            hit: chosen.clone(),
            episodes,
        });
    };

    // Probe the surviving head; a failing probe removes the
    // candidate, never the pick. Each survivor keeps whether its
    // own detail year positively matched Kitsu's.
    let mut probed_ok: Vec<(
        &BrowseHit,
        Vec<crate::scraper::anidb::EpisodeRef>,
        u32,
        bool,
    )> = Vec::new();
    for (h, year_confirmed) in head {
        match client.episodes(&h.slug).await {
            Ok(eps) => {
                let count = u32::try_from(eps.len()).unwrap_or(u32::MAX);
                probed_ok.push((h, eps, count.abs_diff(expected), year_confirmed));
            }
            Err(e) => {
                // A refusal or rate limit is the provider blocking,
                // not this candidate missing: continuing the walk
                // turns one block into a burst of further probes, and
                // the alias walk would repeat the burst per alias.
                // Not-found-shaped statuses are the candidate itself
                // dead — a stale slug 404s — and, like transport
                // failures, only drop the candidate.
                let provider_block = match &e {
                    crate::error::AniError::RateLimited { .. } => true,
                    crate::error::AniError::Upstream { status } => {
                        *status == 403 || *status == 429 || *status >= 500
                    }
                    _ => false,
                };
                if provider_block {
                    return Err(e);
                }
                tracing::debug!(slug = %h.slug, error = ?e, "anidb pick: probe failed, skipping candidate");
            }
        }
    }
    // Every probe failing says nothing about the show — that verdict
    // is transient, never the persistable absence.
    let best_dist = probed_ok
        .iter()
        .map(|(_, _, d, _)| *d)
        .min()
        .ok_or(crate::error::AniError::Network)?;
    if best_dist > ep_count_threshold(expected) {
        // The airing-part rescue: a candidate whose own year matched
        // Kitsu's and whose list is short is what a currently-airing
        // part looks like — Kitsu counts the whole season, the
        // provider only what has aired. Without that positive year
        // evidence a short count stays a miss.
        if let Some(idx) = probed_ok
            .iter()
            .enumerate()
            .filter(|(_, (_, eps, _, confirmed))| *confirmed && eps.len() < expected as usize)
            .min_by_key(|(_, (_, _, d, _))| *d)
            .map(|(i, _)| i)
        {
            let (hit, episodes, _, _) = probed_ok.swap_remove(idx);
            return Ok(PickedShow {
                hit: hit.clone(),
                episodes,
            });
        }
        return Err(crate::error::AniError::NoResults);
    }
    let winner_idx = probed_ok
        .iter()
        .position(|(h, _, d, _)| *d == best_dist && h.title.trim().to_lowercase() == needle)
        .or_else(|| probed_ok.iter().position(|(_, _, d, _)| *d == best_dist))
        .expect("best_dist came from this list");
    let (hit, episodes, _, _) = probed_ok.swap_remove(winner_idx);
    Ok(PickedShow {
        hit: hit.clone(),
        episodes,
    })
}

#[cfg(test)]
#[path = "play_native_test.rs"]
mod tests;
