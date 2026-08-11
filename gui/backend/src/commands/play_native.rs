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

use super::play_native_format::format_survivors;
use super::play_native_numbering::regular_episode_count;
use super::play_native_year::year_filtered;

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
    // Format disproof in both directions, over the RAW list — the
    // badge is free, so incompatible formats never crowd the bounded
    // probe head (see play_native_format).
    let hits = format_survivors(hits, expected, subtype);
    let (head, year_excluded_any) = year_filtered(client, &hits, year).await?;
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
    let mut any_transport_failure = false;
    // Identity carried by transport-DEAD candidates: 0 = exact
    // title, 1 = year-confirmed, 2 = neither. A dead candidate that
    // outranks the eventual winner makes the whole pick transient —
    // the sibling must not win on the strength of weather.
    let rank = |title_matches: bool, confirmed: bool| -> u8 {
        if title_matches {
            0
        } else if confirmed {
            1
        } else {
            2
        }
    };
    let mut best_failed_rank: u8 = u8::MAX;
    for (h, year_confirmed) in head {
        match client.episodes(&h.slug).await {
            Ok(eps) => {
                // Kitsu's expected count excludes recaps; so must
                // the candidate's, or a show is rejected on its own
                // fractional extras.
                let count = regular_episode_count(&eps);
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
                if e.is_provider_block() || matches!(e, crate::error::AniError::GateRefused) {
                    // A block or the gate's own refusal: every
                    // further probe repeats the same answer.
                    return Err(e);
                }
                if !matches!(e, crate::error::AniError::Upstream { .. }) {
                    any_transport_failure = true;
                    let failed = rank(h.title.trim().to_lowercase() == needle, year_confirmed);
                    best_failed_rank = best_failed_rank.min(failed);
                }
                tracing::debug!(slug = %h.slug, error = ?e, "anidb pick: probe failed, skipping candidate");
            }
        }
    }
    // An empty pool splits by what killed the probes: any transport
    // death means nothing was learned (the transient Network), while
    // all-answered not-found means the pool is dead but the provider
    // is healthy — the not-found-shaped verdict, so the breaker never
    // opens on a provider that answered every request. Neither is
    // ever the persistable absence.
    let best_dist =
        probed_ok
            .iter()
            .map(|(_, _, d, _)| *d)
            .min()
            .ok_or(if any_transport_failure {
                crate::error::AniError::Network
            } else {
                crate::error::AniError::Upstream { status: 404 }
            })?;
    if best_dist > ep_count_threshold(expected) {
        // The airing-part rescue: a candidate whose own year matched
        // Kitsu's and whose list is short is what a currently-airing
        // part looks like — Kitsu counts the whole season, the
        // provider only what has aired. Without that positive year
        // evidence a short count stays a miss.
        if let Some(idx) = probed_ok
            .iter()
            .enumerate()
            .filter(|(_, (_, eps, _, confirmed))| {
                *confirmed && regular_episode_count(eps) < expected
            })
            .min_by_key(|(_, (_, _, d, _))| *d)
            .map(|(i, _)| i)
        {
            if best_failed_rank < 1 {
                // An exact-title candidate died unheard; the rescue
                // must not outrank it on weather.
                return Err(crate::error::AniError::Network);
            }
            let (hit, episodes, _, _) = probed_ok.swap_remove(idx);
            return Ok(PickedShow {
                hit: hit.clone(),
                episodes,
            });
        }
        // A rejection is only a clean verdict when every candidate
        // got to answer: a transiently dead probe may have hidden
        // the right show, and NoResults rides the walk into a
        // persistable clean miss. Weather stays weather.
        if any_transport_failure {
            return Err(crate::error::AniError::Network);
        }
        return Err(crate::error::AniError::NoResults);
    }
    // Among the best-distance candidates: an exact title match is
    // the user's own words and stays dominant; below it, a detail
    // year that matched Kitsu's exactly outranks a merely tolerated
    // neighbor; only a full tie falls to provider order (min_by_key
    // keeps the first of equals).
    let winner_idx = probed_ok
        .iter()
        .enumerate()
        .filter(|(_, (_, _, d, _))| *d == best_dist)
        .min_by_key(|(_, (h, _, _, confirmed))| {
            (h.title.trim().to_lowercase() != needle, !*confirmed)
        })
        .map(|(i, _)| i)
        .expect("best_dist came from this list");
    let (winner_title_matches, winner_confirmed) = {
        let (h, _, _, c) = &probed_ok[winner_idx];
        (h.title.trim().to_lowercase() == needle, *c)
    };
    if best_failed_rank < rank(winner_title_matches, winner_confirmed) {
        return Err(crate::error::AniError::Network);
    }
    let (hit, episodes, _, _) = probed_ok.swap_remove(winner_idx);
    Ok(PickedShow {
        hit: hit.clone(),
        episodes,
    })
}

#[cfg(test)]
#[path = "play_native_test.rs"]
mod tests;
