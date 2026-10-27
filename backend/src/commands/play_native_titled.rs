//! The title-aware pick over a provider's browse hits — probing episode
//! counts, layering the filters, and choosing the show; split from
//! [`super`] so each file stays inside the CRAP gate's per-file bar.

use super::*;
use crate::commands::play_native_count_alone::unvouched_counted;
use crate::commands::play_native_dated_name::{may_win, unnamed_dated};
use crate::commands::play_native_single_video::unvouched_single_videos;
use crate::commands::play_native_title_grammar::normalized;
use crate::commands::play_native_versions::{note_dead, versions_of, with_versions};
use crate::commands::play_native_year::PageNames;

/// One pick over `hits`, leaving out of its bounded head every listing
/// `gate` refused.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn pick_over<P: Provider + ?Sized>(
    client: &P,
    hits: &[BrowseHit],
    expected: Option<u32>,
    search_title: &str,
    entry_titles: &[&str],
    year: Option<u32>,
    subtype: Option<&str>,
    gate: &mut crate::commands::play_native_genre::GenreRefusals,
) -> Result<PickedShow> {
    if hits.is_empty() {
        // Nothing to probe: a clean absence of candidates, distinct
        // from probes that failed below.
        return Err(crate::error::AniError::NoResults);
    }
    let entry = EntryTitles::new(entry_titles);
    let needle = normalized(search_title);
    // Format disproof in both directions, over the RAW list — the
    // badge is free, so incompatible formats never crowd the bounded
    // probe head (see play_native_format).
    let mut hits = format_survivors(hits, expected, subtype);
    // A part before the requested entry is not the entry, whichever way
    // the pick would use it: alone, rescued as airing, or heading a
    // stitched chain.
    hits.retain(|h| !precedes_entry(&h.title, entry_titles));
    // The bounded head, by the entry's titles (see
    // play_native_title_verdict::probe_head).
    let (mut hits, refused_dropped) = probe_head(hits, expected.is_some(), entry)?;
    gate.leave_out(&mut hits);
    let mut pages = PageNames::new();
    let filtered = year_filtered(client, &hits, year, &mut pages).await;
    let Some(expected) = expected else {
        let picked = match filtered {
            Ok(head) => pick_without_count(client, &head, &needle, entry).await,
            Err(e) => Err(e),
        };
        return countless_miss(picked, refused_dropped);
    };
    let head = filtered?;
    if head.is_empty() {
        return Err(crate::error::AniError::NoResults);
    }

    // Probe the surviving head; a failing probe removes the
    // candidate, never the pick. Each survivor keeps whether its
    // own detail year positively matched Kitsu's.
    let mut probed_ok: Vec<(&BrowseHit, Vec<EpisodeRef>, u32, bool)> = Vec::new();
    let mut any_transport_failure = false;
    // What the listings that answered not-found say about the pool.
    let mut dead = DeadListings::default();
    // Identity carried by transport-DEAD candidates
    // ([`entry_rank`]), with their provider position: a dead
    // candidate that outranks the eventual winner — or ties an
    // identity-bearing rank from an earlier position, where provider
    // order would have decided for it — makes the whole pick
    // transient. The sibling must not win on the strength of
    // weather.
    let mut best_failed: Option<(u8, usize)> = None;
    let mut positions: Vec<usize> = Vec::new();
    // The transport-dead candidates that could have been picked, which
    // a winner's copy among them leaves unheard (see
    // play_native_versions::copy_died). A copy whose listing answered
    // not-found is positively gone, and is a dead listing like any
    // other.
    let mut dead_copies: Vec<&BrowseHit> = Vec::new();
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
                let could_win = may_win(h, expected, &needle, entry, year, &pages, year_confirmed);
                if !matches!(e, crate::error::AniError::Upstream { .. }) {
                    any_transport_failure = true;
                    best_failed = strongest_dead(
                        best_failed,
                        entry,
                        &h.title,
                        &needle,
                        year_confirmed,
                        pos,
                        could_win,
                    );
                    note_dead(&mut dead_copies, h, could_win, entry);
                } else {
                    // Answered not-found: a stale listing that may have
                    // been the entry's, if it could have won.
                    dead.note(entry, &h.title, could_win);
                }
                tracing::debug!(slug = %h.slug, error = ?e, "pick: probe failed, skipping candidate");
            }
        }
    }
    // The entry's own titles over what the probes heard: a sibling
    // named for another season or part is scored out, and a listing
    // that spans this entry and the next is cut to this one's
    // episodes (see play_native_wide_listing). Before the split
    // stitching below, which still sees every part.
    let refused_a_fit = fit_to_entry(&mut probed_ok, expected, entry);
    // A single video's count fits every other single video: only its
    // name or its year makes it the entry (see
    // play_native_single_video).
    let refused_a_fit =
        unvouched_single_videos(&mut probed_ok, expected, &needle, entry) || refused_a_fit;
    // A count with no year fits many shows: only its name makes a
    // candidate the entry (see play_native_count_alone). A dated
    // entry, or one a single video long, is not read here.
    let refused_a_fit =
        unvouched_counted(&mut probed_ok, expected, &needle, entry, year) || refused_a_fit;
    // A year and a count fit many shows too: with both, only a name
    // makes a candidate the entry (see play_native_dated_name).
    let refused_a_fit =
        unnamed_dated(&mut probed_ok, expected, &needle, entry, year, &pages) || refused_a_fit;
    // An empty pool splits by what killed the probes: any transport
    // death means nothing was learned (the transient Network), while
    // all-answered not-found means the pool is dead but the provider
    // is healthy — the not-found-shaped verdict, so the breaker never
    // opens on a provider that answered every request. Neither is
    // ever the persistable absence.
    let best_dist = probed_ok
        .iter()
        .map(|(_, _, d, _)| *d)
        .min()
        .ok_or_else(|| dead.verdict(any_transport_failure))?;
    // One Kitsu entry the provider lists as several shows: every part
    // was probed, so stitching them costs nothing. Only with every
    // candidate heard — a dead probe may have been one of the parts.
    if !any_transport_failure {
        if let Some(picked) =
            crate::commands::play_native_split::stitched(&probed_ok, expected, best_dist, entry)
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
            .filter(|(_, row)| rescuable(row, expected))
            // Distance first, then the user's own words and the
            // entry's part — the dominance winner selection keeps —
            // with provider order as the final tie (min_by_key keeps
            // the first of equals).
            .min_by_key(|(_, (h, _, d, _))| {
                (
                    *d,
                    normalized(&h.title) != needle,
                    !entry.part_agrees(&h.title),
                )
            })
            .map(|(i, _)| i)
        {
            let (h, _, _, c) = &probed_ok[idx];
            let rescue_rank = entry_rank(entry, &h.title, &needle, *c);
            if dead_outranks(best_failed, rescue_rank, positions[idx]) {
                // An identity-bearing candidate died unheard; the
                // rescue must not outrank it on weather.
                return Err(crate::error::AniError::Network);
            }
            let (hit, episodes, _, _) = probed_ok.swap_remove(idx);
            return Ok(PickedShow::listed(hit.clone(), episodes));
        }
        // A rejection is only a clean verdict when every candidate
        // got to answer: a transiently dead probe may have hidden
        // the right show, and NoResults rides the walk into a
        // persistable clean miss. Weather stays weather.
        return Err(rejection(
            any_transport_failure,
            dead.refused_or(refused_a_fit),
            dead.might_decide(&probed_ok, expected, entry),
        ));
    }
    let (winner_idx, winner_rank) = select_winner(&probed_ok, best_dist, &needle, entry);
    if winner_unheard(
        best_failed,
        winner_rank,
        positions[winner_idx],
        &dead_copies,
        probed_ok[winner_idx].0,
    ) {
        return Err(crate::error::AniError::Network);
    }
    // Copies of the winner under another version tag are versions of
    // this one pick (see play_native_versions).
    let versions = versions_of(&probed_ok, winner_idx, best_dist);
    let (hit, episodes, _, _) = probed_ok.swap_remove(winner_idx);
    Ok(with_versions(
        PickedShow::listed(hit.clone(), episodes),
        versions,
    ))
}
