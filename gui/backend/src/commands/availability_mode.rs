//! How much of a listing the requested audio mode actually covers.
//!
//! The provider's search and episode listing are mode-independent —
//! only an episode's languages row says whether a dub exists. Dubs
//! also trail subs: a show four episodes in may carry `eng` rows for
//! the first two only. So availability cannot read "a dub exists"
//! off one row and then cap from the whole listing; the cap has to
//! describe the rows the requested mode has, or the strip unlocks
//! episodes whose playback and download fail.

use crate::error::{AniError, Result};
use crate::scraper::anidb::{AnidbClient, AnidbFetch, EpisodeRef};

/// How many leading rows of `episodes` carry `mode`, or `None` when
/// no live row answered about it at all.
///
/// `Some(0)` means a live row said the mode is absent — the answered
/// negative availability may cache. `None` means every row the
/// search touched was missing, which says nothing about the mode and
/// must not be persisted as absence.
///
/// The listing's order is the provider's own, and a provider dubs in
/// that order, so the covered rows are a prefix: the boundary is
/// found by bisection rather than by asking every row, which keeps a
/// 1000-episode show at ~10 requests instead of 1000. Two of those
/// requests carry the common cases outright — a last row that has
/// the mode means the whole listing does (one request for every sub
/// probe and every fully dubbed show), and a first row without it
/// means the mode is absent (two).
///
/// Missing rows are stepped over rather than believed in either
/// direction, but only [`SCAN_BUDGET`] of them per search step: a
/// listing that answers not-found everywhere is not worth one
/// request per episode to confirm, so the search gives up and
/// reports no verdict instead of scanning to the end.
///
/// # Errors
/// The transport's own failures, as in [`AnidbClient::has_mode`].
/// An answered-not-found row is NOT an error — see [`row_mode`].
pub(crate) async fn mode_prefix_len<F: AnidbFetch>(
    client: &AnidbClient<F>,
    episodes: &[EpisodeRef],
    mode: &str,
) -> Result<Option<usize>> {
    if episodes.is_empty() {
        return Ok(Some(0));
    }
    // The highest row that answered. Its verdict decides whether
    // there is a boundary to look for at all.
    let Some((hi, hi_covered)) =
        decisive(client, episodes, mode, (0..episodes.len()).rev()).await?
    else {
        return Ok(None);
    };
    if hi_covered {
        // A live row at the tail carries the mode, so the whole
        // listing does — a dead row above it cannot subtract from
        // what that answer proved.
        return Ok(Some(episodes.len()));
    }
    let Some((lo, lo_covered)) = decisive(client, episodes, mode, 0..hi).await? else {
        return Ok(None);
    };
    if !lo_covered {
        return Ok(Some(0));
    }
    // Invariant: `lo` answered covered, `hi` answered uncovered.
    // Each turn halves the gap or steps past a dead run, so the loop
    // ends with the boundary between them.
    let mut lo = lo;
    let mut hi = hi;
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        match decisive(client, episodes, mode, mid..hi).await? {
            Some((j, true)) => lo = j,
            Some((j, false)) => hi = j,
            // Nothing between mid and hi answered: no live row
            // proved coverage up there, so the search keeps only
            // what one already did.
            None => hi = mid,
        }
    }
    Ok(Some(lo + 1))
}

/// How many missing rows one search step will step over before
/// giving up. Bounded deliberately: a listing that answers not-found
/// everywhere would otherwise cost one request per episode to learn
/// nothing.
const SCAN_BUDGET: usize = 8;

/// The first row in `idxs` that answers about `mode`, with its
/// verdict. `None` when every row it touched was missing (or the
/// budget ran out first).
async fn decisive<F: AnidbFetch, I: Iterator<Item = usize>>(
    client: &AnidbClient<F>,
    episodes: &[EpisodeRef],
    mode: &str,
    idxs: I,
) -> Result<Option<(usize, bool)>> {
    for (touched, i) in idxs.enumerate() {
        if touched >= SCAN_BUDGET {
            break;
        }
        if let Some(covered) = row_mode(client, episodes[i].id, mode).await? {
            return Ok(Some((i, covered)));
        }
    }
    Ok(None)
}

/// Whether one row carries `mode`, or `None` when the row itself is
/// missing.
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
