//! Native play resolution — the picker half, over any provider. Replaces the "compute a `-S` index and hope the script's
//! search returns the same list" coupling with a direct pick over the
//! client's own results.
//!
//! The browse page carries titles only, so episode counts come from
//! one episodes call per considered candidate. The probe set is
//! bounded: real queries put the right show in the first few hits,
//! and every probe is an upstream request. Within the probed set the
//! proven layers from the provider picker apply — episode-count
//! distance with the `max(3, 10%)` threshold, then an exact-name
//! tie-break — minus the year filter, which waits until a live
//! capture confirms where the browse markup carries a year.

use crate::error::Result;
use crate::scraper::provider::{BrowseHit, EpisodeRef, Provider};

use super::play_native_choice::{entry_rank, pick_without_count, select_winner, strongest_dead};
use super::play_native_format::format_survivors;
use super::play_native_numbering::regular_episode_count;
use super::play_native_part_title::precedes_entry;
use super::play_native_title_marker::EntryTitles;
use super::play_native_title_verdict::{countless_miss, probe_head, rejection, rescuable};
use super::play_native_wide_listing::fit_to_entry;
use super::play_native_year::year_filtered;

#[path = "play_native_titled.rs"]
mod titled;
pub use titled::pick_candidate_titled;

/// How many browse hits the entry's titles admit get an episodes
/// probe. Beyond this the match was not a match; the request budget
/// is better spent on the next alias. With a count, the refused hits
/// among the first this many are probed besides, as evidence about
/// the pool — at most twice this many probes per pool (see
/// `play_native_title_verdict::probe_head`).
pub const MAX_PROBED_CANDIDATES: usize = 5;

/// A picked show: the hit plus the episode list the probe already
/// paid for, so the caller never re-fetches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedShow {
    /// The winning browse hit.
    pub hit: BrowseHit,
    /// The show's episodes, as returned by the probe.
    pub episodes: Vec<EpisodeRef>,
}

/// Whether a transport-dead candidate outranks the winner: a
/// strictly stronger identity always does, and an equal
/// identity-bearing rank does when the dead candidate came first —
/// provider order would have decided the tie for it. Plain
/// no-identity failures never block; provider order among garbage
/// hits is weak evidence, and the probe-skip behavior exists for
/// exactly that pool.
fn dead_outranks(best_failed: Option<(u8, usize)>, winner_rank: u8, winner_pos: usize) -> bool {
    let Some((rank, pos)) = best_failed else {
        return false;
    };
    let identity = rank == 0 || rank % 2 == 1;
    rank < winner_rank || (rank == winner_rank && identity && pos < winner_pos)
}

/// Distance tolerance: long-running shows get proportional slack,
/// short shows a hard floor of 3 — the same rule the provider picker
/// converged on after the sibling-mispick rounds.
pub fn ep_count_threshold(expected: u32) -> u32 {
    (expected / 10).max(3)
}

/// Pick the show a query meant from browse `hits`, using Kitsu's
/// `expected` episode count and premiere `year` when known.
///
/// - Considers at most [`MAX_PROBED_CANDIDATES`] hits the entry's
///   titles admit — plus, with a count, the refused ones among the
///   first [`MAX_PROBED_CANDIDATES`] as evidence about the pool.
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
/// - With `expected = None`: an exact title match wins; else a
///   candidate whose part agrees with the entry's comes before one
///   naming another part, and within each a matched year before the
///   rest, the provider's order breaking ties — only a year-matched
///   candidate when the year disproved part of the pool (see
///   `pick_without_count`).
/// - A candidate the searched title names a later part of ("X" when
///   asked for "X Season 2") is the season before, and never picked;
///   [`pick_candidate_titled`] reads every title the entry goes by.
///   So is the entry before this one ("Show" for "Show 2"). Each is
///   another entry of the same show, so a pool rejected after it is
///   still a clean miss.
/// - A candidate the entry's titles do not admit — named for a
///   season or part the entry is not — is never picked (with a
///   count it is still probed); a pool rejected for that alone is
///   not a clean miss; a listing that spans this entry and
///   the next is cut to this entry's episodes (see
///   `play_native_wide_listing`).
/// - Probe errors skip the candidate rather than abort the pick; a
///   pick only fails when no probed candidate survives.
///
/// # Errors
/// [`crate::error::AniError::NoResults`] when `hits` is empty or no
/// candidate survives the threshold; the answered dead end
/// `Upstream { status: 404 }` when the titles refused a candidate the
/// count accepted, or without a count refused a hit and nothing else
/// was picked.
pub async fn pick_candidate<P: Provider + ?Sized>(
    client: &P,
    hits: &[BrowseHit],
    expected: Option<u32>,
    search_title: &str,
    year: Option<u32>,
    subtype: Option<&str>,
) -> Result<PickedShow> {
    pick_candidate_titled(
        client,
        hits,
        expected,
        search_title,
        &[search_title],
        year,
        subtype,
    )
    .await
}

#[cfg(test)]
#[path = "play_native_test.rs"]
mod tests;
