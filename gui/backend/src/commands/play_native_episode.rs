//! The episode half of a native resolution: map the requested
//! per-entry number onto the picked show's listing (integer via the
//! continuation offset, decimal via the provider's own display tag)
//! and chase it down to a master-playlist URL. Split from the walk
//! for the per-file complexity bar.

use crate::error::AniError;
use crate::scraper::anidb::{AnidbClient, AnidbFetch};

use super::play_native::PickedShow;
use super::play_native_numbering::numbering_offset;
use super::play_native_resolve::NativeError;

/// Resolve the requested episode within a picked show down to the
/// master URL. Split from the walk for the per-file complexity bar
/// and because the orchestrator's cache-hit path may someday reuse
/// it. The request's number is per-entry; the provider's listing may
/// be cumulative — [`numbering_offset`] bridges the two.
///
/// # Errors
/// `NativeError` (never `clean_miss`): the show matched, so nothing
/// here is evidence of absence.
pub async fn resolve_episode<F: AnidbFetch>(
    client: &AnidbClient<F>,
    picked: &PickedShow,
    episode: &str,
    mode: &str,
    quality: &str,
) -> std::result::Result<String, NativeError> {
    let dead_end = |error: AniError| NativeError {
        error,
        clean_miss: false,
    };
    let ep = match episode.trim().parse::<u32>() {
        Ok(n) => {
            let n = n.saturating_add(numbering_offset(&picked.episodes));
            picked.episodes.iter().find(|e| e.number == n)
        }
        // Not an integer: the fractional tags availability
        // advertises ("1061.5" recaps) match the provider's own
        // display tag verbatim — decimal tags are absolute, so the
        // continuation offset never applies to them.
        Err(_) => picked
            .episodes
            .iter()
            .find(|e| e.number2.as_deref() == Some(episode.trim())),
    }
    .ok_or_else(|| dead_end(AniError::NoResults))?;
    let master = client
        .master_playlist_url(ep.id, mode)
        .await
        .map_err(dead_end)?;
    // The quality step is soft only on a served playlist that lacks
    // the height; a failed master fetch is the episode failing.
    client
        .quality_stream_url(&master, quality)
        .await
        .map_err(dead_end)
}

#[cfg(test)]
#[path = "play_native_episode_test.rs"]
mod tests;
