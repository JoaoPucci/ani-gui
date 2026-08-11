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

/// How many leading rows of `episodes` carry `mode`.
///
/// `0` means the mode is absent — the answered negative availability
/// may cache. The listing's order is the provider's own, and a
/// provider dubs in that order, so the covered rows are a prefix:
/// the boundary is found by bisection rather than by asking every
/// row, which keeps a 1000-episode show at ~10 requests instead of
/// 1000. Two of those requests carry the common cases outright — a
/// last row that has the mode means the whole listing does (one
/// request for every sub probe and every fully dubbed show), and a
/// first row without it means the mode is absent (two).
///
/// # Errors
/// The transport's own failures, as in [`AnidbClient::has_mode`].
/// An answered-not-found row is NOT an error and NOT a verdict about
/// the mode — see [`row_has_mode`].
pub(crate) async fn mode_prefix_len<F: AnidbFetch>(
    client: &AnidbClient<F>,
    episodes: &[EpisodeRef],
    mode: &str,
) -> Result<usize> {
    let Some(last) = episodes.last() else {
        return Ok(0);
    };
    if row_has_mode(client, last.id, mode).await? {
        return Ok(episodes.len());
    }
    let first = &episodes[0];
    if !row_has_mode(client, first.id, mode).await? {
        return Ok(0);
    }
    // Invariant: `lo` is covered, `hi` is not, and both are indices
    // into `episodes`. Each turn halves the gap, so the loop ends
    // with the boundary between them.
    let mut lo = 0usize;
    let mut hi = episodes.len() - 1;
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if row_has_mode(client, episodes[mid].id, mode).await? {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(lo + 1)
}

/// Whether one row carries `mode`.
///
/// An answered non-block status is a verdict about the ROW — a stale
/// episode id the provider no longer serves — not about the mode, so
/// it reads as covered rather than truncating the cap at a dead row.
/// Weather propagates: the caller feeds it to the breaker and
/// persists nothing.
async fn row_has_mode<F: AnidbFetch>(
    client: &AnidbClient<F>,
    episode_id: u64,
    mode: &str,
) -> Result<bool> {
    match client.has_mode(episode_id, mode).await {
        Ok(has) => Ok(has),
        Err(e) if matches!(e, AniError::Upstream { .. }) && !e.is_provider_block() => Ok(true),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "availability_mode_test.rs"]
mod tests;
