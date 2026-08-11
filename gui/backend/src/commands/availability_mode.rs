//! Whether a show carries the requested audio mode.
//!
//! The provider's search and episode listing are mode-independent —
//! only an episode's languages row says whether a dub exists — so
//! availability has to ask one before it may cache `available` under
//! a dub key. This module asks exactly that question, at the show's
//! granularity, and no more.
//!
//! # Why not per-episode
//!
//! Dubs trail subs, so a show can carry `eng` rows for its first few
//! episodes only, and a cap describing the dubbed prefix would be
//! strictly better information. Three review rounds went into
//! deriving one, and each fix exposed the next place where a row the
//! search never looked at got counted: the tail, then the front,
//! then anywhere below a bisected boundary. That is not a sequence
//! of bugs, it is the shape of the problem — which episodes carry a
//! dub is per-episode data, and no sub-linear probe can vouch for
//! rows it never fetched. Proving a cap needs one request per
//! episode, which a 1000-episode listing cannot afford and the
//! resolver's deadline would not survive.
//!
//! So the claim is cut to match the evidence: this says whether the
//! show has the mode, the cap keeps coming from the listing, and a
//! dubless episode inside a partly-dubbed show surfaces at playback.
//!
//! That last part is a step back from allanime, which returned
//! per-mode episode arrays in the show payload and so capped each
//! mode exactly for one fetch. anidb publishes no audio on the
//! listing at all, which is what makes the same answer cost a
//! request per episode. `docs/deferred-work.md` carries the
//! refinement and what a real fix would need.

use crate::error::{AniError, Result};
use crate::scraper::anidb::{AnidbClient, AnidbFetch, EpisodeRef};

/// How many missing rows the search steps over before giving up.
/// Bounded deliberately: a listing that answers not-found everywhere
/// is not worth one request per episode to confirm.
const SCAN_BUDGET: usize = 8;

/// Whether the show carries `mode`, or `None` when no row answered.
///
/// `Some(false)` is an answered absence and availability may cache
/// it. `None` means every row the search touched was missing, which
/// says nothing about the mode either way — the caller surfaces that
/// and persists nothing, so a stale row costs a re-probe instead of
/// a wrong cache row for the row's whole TTL.
///
/// Rows are asked in listing order because dubs land in that order:
/// the first row is the one a dubbed show is most likely to have,
/// and the one a viewer starts from. A missing row is stepped over —
/// it is a verdict about that row, not about the mode — up to
/// [`SCAN_BUDGET`] of them.
///
/// # Errors
/// The transport's own failures, as in [`AnidbClient::has_mode`].
/// An answered-not-found row is NOT an error — it is the unknown
/// verdict above.
pub(crate) async fn mode_present<F: AnidbFetch>(
    client: &AnidbClient<F>,
    episodes: &[EpisodeRef],
    mode: &str,
) -> Result<Option<bool>> {
    for episode in episodes.iter().take(SCAN_BUDGET) {
        if let Some(present) = row_mode(client, episode.id, mode).await? {
            return Ok(Some(present));
        }
    }
    // A listing with no rows carries nothing to play, in any mode.
    Ok(episodes.is_empty().then_some(false))
}

/// Whether one row carries `mode`, or `None` when the row itself
/// answered nothing.
///
/// An answered non-block status is a verdict about the ROW — a stale
/// episode id the provider no longer serves — and says nothing about
/// the mode in either direction. Weather propagates: the caller
/// feeds it to the breaker and persists nothing.
async fn row_mode<F: AnidbFetch>(
    client: &AnidbClient<F>,
    episode_id: u64,
    mode: &str,
) -> Result<Option<bool>> {
    match client.has_mode(episode_id, mode).await {
        Ok(has) => Ok(Some(has)),
        Err(e) if matches!(e, AniError::Upstream { .. }) && !e.is_provider_block() => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "availability_mode_test.rs"]
mod tests;
