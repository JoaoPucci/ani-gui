//! The title-aware pick over a provider's browse hits — probing episode
//! counts, layering the filters, and choosing the show; split from
//! [`super`] so each file stays inside the CRAP gate's per-file bar.

use super::*;

/// [`pick_candidate`] told every title the requested entry goes by —
/// the canonical title and its fallbacks, not only the one searched —
/// which the split-entry detection reads to tell the entry from its
/// parts.
///
/// # Errors
/// As [`pick_candidate`].
pub async fn pick_candidate_titled<P: Provider + ?Sized>(
    client: &P,
    hits: &[BrowseHit],
    expected: Option<u32>,
    search_title: &str,
    entry_titles: &[&str],
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
    let mut hits = format_survivors(hits, expected, subtype);
    // A part before the requested entry is not the entry, whichever way
    // the pick would use it: alone, rescued as airing, or heading a
    // stitched chain.
    hits.retain(|h| !precedes_entry(&h.title, entry_titles));
    let (head, year_excluded_any) = year_filtered(client, &hits, year).await?;
    if head.is_empty() {
        return Err(crate::error::AniError::NoResults);
    }

    let Some(expected) = expected else {
        return pick_without_count(client, &head, &needle, year_excluded_any).await;
    };

    // Probe the surviving head; a failing probe removes the
    // candidate, never the pick. Each survivor keeps whether its
    // own detail year positively matched Kitsu's.
    let mut probed_ok: Vec<(&BrowseHit, Vec<EpisodeRef>, u32, bool)> = Vec::new();
    let mut any_transport_failure = false;
    // Identity carried by transport-DEAD candidates
    // ([`identity_rank`]), with their provider position: a dead
    // candidate that outranks the eventual winner — or ties an
    // identity-bearing rank from an earlier position, where provider
    // order would have decided for it — makes the whole pick
    // transient. The sibling must not win on the strength of
    // weather.
    let mut best_failed: Option<(u8, usize)> = None;
    let mut positions: Vec<usize> = Vec::new();
    for (pos, (h, year_confirmed)) in head.into_iter().enumerate() {
        match client.episodes(&h.slug).await {
            Ok(eps) => {
                // Kitsu's expected count excludes recaps; so must
                // the candidate's, or a show is rejected on its own
                // fractional extras.
                let count = regular_episode_count(&eps);
                probed_ok.push((h, eps, count.abs_diff(expected), year_confirmed));
                positions.push(pos);
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
                    let failed =
                        identity_rank(h.title.trim().to_lowercase() == needle, year_confirmed);
                    if best_failed.is_none_or(|best| (failed, pos) < best) {
                        best_failed = Some((failed, pos));
                    }
                }
                tracing::debug!(slug = %h.slug, error = ?e, "pick: probe failed, skipping candidate");
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
    // One Kitsu entry the provider lists as several shows: every part
    // was probed, so stitching them costs nothing. Only with every
    // candidate heard — a dead probe may have been one of the parts.
    if !any_transport_failure {
        if let Some(picked) =
            crate::commands::play_native_split::stitched(&probed_ok, expected, best_dist)
        {
            return Ok(picked);
        }
    }
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
            // Distance first, then the user's own words — the same
            // dominance winner selection keeps — with provider order
            // as the final tie (min_by_key keeps the first of
            // equals).
            .min_by_key(|(_, (h, _, d, _))| (*d, h.title.trim().to_lowercase() != needle))
            .map(|(i, _)| i)
        {
            let (h, _, _, c) = &probed_ok[idx];
            let rescue_rank = identity_rank(h.title.trim().to_lowercase() == needle, *c);
            if dead_outranks(best_failed, rescue_rank, positions[idx]) {
                // An identity-bearing candidate died unheard; the
                // rescue must not outrank it on weather.
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
    let (winner_idx, winner_rank) = select_winner(&probed_ok, best_dist, &needle);
    if dead_outranks(best_failed, winner_rank, positions[winner_idx]) {
        return Err(crate::error::AniError::Network);
    }
    let (hit, episodes, _, _) = probed_ok.swap_remove(winner_idx);
    Ok(PickedShow {
        hit: hit.clone(),
        episodes,
    })
}
