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

use super::play_native_choice::{identity_rank, pick_without_count, select_winner};
use super::play_native_format::format_survivors;
use super::play_native_numbering::regular_episode_count;
use super::play_native_part_title::precedes_entry;
use super::play_native_year::year_filtered;

#[path = "play_native_titled.rs"]
mod titled;
pub use titled::pick_candidate_titled;

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
    rank < winner_rank || (rank == winner_rank && rank <= 1 && pos < winner_pos)
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
/// - A candidate the searched title names a later part of ("X" when
///   asked for "X Season 2") is the season before, and never picked;
///   [`pick_candidate_titled`] reads every title the entry goes by.
/// - Probe errors skip the candidate rather than abort the pick; a
///   pick only fails when no probed candidate survives.
///
/// # Errors
/// [`crate::error::AniError::NoResults`] when `hits` is empty or no
/// candidate survives the threshold.
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
