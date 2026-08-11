//! The episode half of a native resolution: map the requested
//! per-entry number onto the picked show's listing (integer via the
//! continuation offset, decimal via the provider's own display tag)
//! and chase it down to a master-playlist URL. Split from the walk
//! for the per-file complexity bar.

use crate::error::AniError;
use crate::scraper::anidb::{AnidbClient, AnidbFetch};

use super::play_native::PickedShow;
use super::play_native_numbering::{numbering_offset, provider_fraction};
use super::play_native_resolve::NativeError;

/// What one failed episode chain means for the surrounding alias
/// walk. Classified here so the walk stays a loop over verdicts.
pub(super) enum ChainOutcome {
    /// Blocks and gate refusals: every further request repeats the
    /// same answer, so the walk ends with the error's identity.
    Stop(NativeError),
    /// Answered dead ends — missing episode, language, embed or
    /// playlist on a stale candidate: the next alias may carry the
    /// real show.
    DeadEnd,
    /// Transport weather: proves nothing, stays transient.
    Transient,
}

/// Classify a failed [`resolve_episode`] for the walk.
pub(super) fn classify_chain_failure(ne: NativeError) -> ChainOutcome {
    if ne.error.is_provider_block() || matches!(ne.error, AniError::GateRefused) {
        ChainOutcome::Stop(ne)
    } else if matches!(ne.error, AniError::NoResults | AniError::Upstream { .. }) {
        ChainOutcome::DeadEnd
    } else {
        ChainOutcome::Transient
    }
}

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
        failed_at: None,
    };
    // A row's identity is its effective display tag — number2 when
    // present, the integer slot otherwise. A recap occupying slot 4
    // under tag "3.5" is not episode 4, and the true episode 4 may
    // sit a slot later under tag "4". Integer requests map through
    // the continuation offset into the same display space the tags
    // live in; fractional requests carry the provider's tag
    // verbatim, so the offset never applies to them.
    let offset = numbering_offset(&picked.episodes);
    let target = match episode.trim().parse::<u32>() {
        Ok(n) => n.saturating_add(offset).to_string(),
        // Fractional requests arrive in the same per-entry numbering
        // the advertisement translated into; the click translates
        // back ([`provider_fraction`]).
        Err(_) => provider_fraction(episode.trim(), offset),
    };
    let ep = picked
        .episodes
        .iter()
        .find(|e| match e.number2.as_deref() {
            Some(tag) => tag == target,
            None => e.number.to_string() == target,
        })
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
