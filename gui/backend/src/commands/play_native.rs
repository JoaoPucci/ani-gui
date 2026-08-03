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
/// candidate. Unknown years (a 404ing detail page, a page without a
/// season link) never exclude — the year is a soft hint. When every
/// known year disagrees the data is suspect, so the unfiltered head
/// comes back and count decides as before; the flag says whether the
/// filter actually discriminated.
async fn year_filtered<'a, F: AnidbFetch>(
    client: &AnidbClient<F>,
    hits: &'a [BrowseHit],
    year: Option<u32>,
) -> (Vec<&'a BrowseHit>, bool) {
    let head: Vec<&BrowseHit> = hits.iter().take(MAX_PROBED_CANDIDATES).collect();
    let Some(year) = year else {
        return (head, false);
    };
    if head.len() < 2 {
        return (head, false);
    }
    let mut kept = Vec::with_capacity(head.len());
    for h in &head {
        let got = client.detail_year(&h.slug).await;
        if got.is_none_or(|y| y.abs_diff(year) <= 1) {
            kept.push(*h);
        }
    }
    if kept.is_empty() {
        return (head, false);
    }
    let filtered = kept.len() < head.len();
    (kept, filtered)
}

/// Pick the show a query meant from browse `hits`, using Kitsu's
/// `expected` episode count and premiere `year` when known.
///
/// - Considers at most [`MAX_PROBED_CANDIDATES`] hits.
/// - With `year = Some(y)`: candidates whose detail page names a
///   premiere year more than one off `y` are excluded before any
///   scoring — the identity signal that separates cour and
///   franchise siblings whose counts tie. Unknown years pass; a
///   pool whose known years all disagree keeps every candidate.
/// - With `expected = Some(n)`: best episode-count distance wins,
///   rejected when the best distance exceeds [`ep_count_threshold`];
///   ties prefer an exact (case-insensitive, trimmed) title match on
///   `search_title`. When the year filter discriminated and no
///   survivor sits within the threshold, a survivor with fewer
///   episodes than expected still wins — an airing part has aired
///   fewer episodes than the total Kitsu knows is coming.
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
) -> Result<PickedShow> {
    if hits.is_empty() {
        // Nothing to probe: a clean absence of candidates, distinct
        // from probes that failed below.
        return Err(crate::error::AniError::NoResults);
    }
    let needle = search_title.trim().to_lowercase();
    let (head, year_discriminated) = year_filtered(client, hits, year).await;

    let Some(expected) = expected else {
        // No count signal: an exact title beats positional order,
        // else the provider's own (filtered) ranking stands. The
        // single probe still runs so the caller gets the episode
        // list it needs.
        let chosen = head
            .iter()
            .find(|h| h.title.trim().to_lowercase() == needle)
            .copied()
            .or_else(|| head.first().copied())
            .ok_or(crate::error::AniError::NoResults)?;
        let episodes = client.episodes(&chosen.slug).await?;
        return Ok(PickedShow {
            hit: chosen.clone(),
            episodes,
        });
    };

    // Probe the surviving head; a failing probe removes the
    // candidate, never the pick.
    let mut probed_ok: Vec<(&BrowseHit, Vec<crate::scraper::anidb::EpisodeRef>, u32)> = Vec::new();
    for h in head {
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
        // The airing-part rescue: when the year picked this pool, a
        // short count is what a currently-airing part looks like —
        // Kitsu counts the whole season, the provider only what has
        // aired. Without year evidence a short count stays a miss.
        if year_discriminated {
            if let Some(idx) = probed_ok
                .iter()
                .enumerate()
                .filter(|(_, (_, eps, _))| eps.len() < expected as usize)
                .min_by_key(|(_, (_, _, d))| *d)
                .map(|(i, _)| i)
            {
                let (hit, episodes, _) = probed_ok.swap_remove(idx);
                return Ok(PickedShow {
                    hit: hit.clone(),
                    episodes,
                });
            }
        }
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
