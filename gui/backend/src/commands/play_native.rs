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

/// Pick the show a query meant from browse `hits`, using Kitsu's
/// `expected` episode count when known.
///
/// - Probes at most [`MAX_PROBED_CANDIDATES`] hits via `episodes`.
/// - With `expected = Some(n)`: best episode-count distance wins,
///   rejected when the best distance exceeds [`ep_count_threshold`];
///   ties prefer an exact (case-insensitive, trimmed) title match on
///   `search_title`.
/// - With `expected = None`: an exact title match wins, else the
///   first hit — positional order is the provider's own ranking.
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
    _year: Option<u32>,
) -> Result<PickedShow> {
    if hits.is_empty() {
        // Nothing to probe: a clean absence of candidates, distinct
        // from probes that failed below.
        return Err(crate::error::AniError::NoResults);
    }
    let needle = search_title.trim().to_lowercase();
    let probed = hits.iter().take(MAX_PROBED_CANDIDATES);

    let Some(expected) = expected else {
        // No count signal: an exact title beats positional order,
        // else the provider's own ranking stands. The single probe
        // still runs so the caller gets the episode list it needs.
        let chosen = hits
            .iter()
            .take(MAX_PROBED_CANDIDATES)
            .find(|h| h.title.trim().to_lowercase() == needle)
            .or_else(|| hits.first())
            .ok_or(crate::error::AniError::NoResults)?;
        let episodes = client.episodes(&chosen.slug).await?;
        return Ok(PickedShow {
            hit: chosen.clone(),
            episodes,
        });
    };

    // Probe the bounded head of the list; a failing probe removes
    // the candidate, never the pick.
    let mut probed_ok: Vec<(&BrowseHit, Vec<crate::scraper::anidb::EpisodeRef>, u32)> = Vec::new();
    for h in probed {
        match client.episodes(&h.slug).await {
            Ok(eps) => {
                let count = u32::try_from(eps.len()).unwrap_or(u32::MAX);
                probed_ok.push((h, eps, count.abs_diff(expected)));
            }
            Err(e) => {
                tracing::debug!(slug = %h.slug, error = ?e, "anidb pick: probe failed, skipping candidate");
            }
        }
    }
    // Every probe failing says nothing about the show — that verdict
    // is transient, never the persistable absence.
    let best_dist = probed_ok
        .iter()
        .map(|(_, _, d)| *d)
        .min()
        .ok_or(crate::error::AniError::Network)?;
    if best_dist > ep_count_threshold(expected) {
        return Err(crate::error::AniError::NoResults);
    }
    let winner_idx = probed_ok
        .iter()
        .position(|(h, _, d)| *d == best_dist && h.title.trim().to_lowercase() == needle)
        .or_else(|| probed_ok.iter().position(|(_, _, d)| *d == best_dist))
        .expect("best_dist came from this list");
    let (hit, episodes, _) = probed_ok.swap_remove(winner_idx);
    Ok(PickedShow {
        hit: hit.clone(),
        episodes,
    })
}

#[cfg(test)]
#[path = "play_native_test.rs"]
mod tests;
