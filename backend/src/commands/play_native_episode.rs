//! The episode half of a native resolution: map the requested
//! per-entry number onto the picked show's listing (integer via the
//! continuation offset, decimal via the provider's own display tag)
//! and chase it down to a master-playlist URL. Split from the walk
//! for the per-file complexity bar.

use crate::error::AniError;
use crate::scraper::provider::{Provider, SubtitleTrack};

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
    /// playlist on a stale candidate, a not-found-shaped status: the
    /// next alias may carry the real show.
    DeadEnd,
    /// The stream server not serving, or an upstream status that is
    /// neither a block nor absence: proves nothing about the show, so
    /// the walk goes on, and the error is kept under its own name —
    /// an exhausted walk surfaces an answered status as the status,
    /// which the breaker reads as the provider answering rather than
    /// as distress.
    Transient(NativeError),
}

/// Classify a failed [`resolve_episode`] for the walk.
pub(super) fn classify_chain_failure(ne: NativeError) -> ChainOutcome {
    if ne.error.is_provider_block() || matches!(ne.error, AniError::GateRefused) {
        ChainOutcome::Stop(ne)
    } else if matches!(ne.error, AniError::EpisodeUnavailable | AniError::NoResults)
        || ne.error.is_not_found_shaped()
    {
        ChainOutcome::DeadEnd
    } else {
        ChainOutcome::Transient(ne)
    }
}

/// The episode step's verdict for a failure it met: an answered dead
/// end — the listing without the episode, the embed step without a
/// stream, a page the host answered with a not-found-shaped status,
/// 404 or 410 — is the episode's own verdict, since the show was
/// picked and only this episode is missing. A failure that is not an
/// answer — the connection to a host dropped, a host timed out, a
/// page or payload or playlist the client could not read — is the
/// stream server not serving ([`AniError::StreamServerFailed`]):
/// past the searches that found the show it is neither the user's
/// connection nor the app failing to read the provider, and told
/// either the user would be misled. The provider's client names the
/// stage and host when it can; one that reaches this step unnamed is
/// named for the chain as a whole. A provider block, a gate refusal
/// and any other upstream status pass through as they are; they say
/// nothing about the episode — a 400 or 401 is the provider rejecting
/// this request, and told to try another episode on it the user
/// would be misled too.
pub(super) fn episode_verdict(error: AniError) -> AniError {
    if matches!(error, AniError::NoResults) || error.is_not_found_shaped() {
        AniError::EpisodeUnavailable
    } else if matches!(
        error,
        AniError::Network | AniError::Timeout | AniError::ParseFailed { .. }
    ) {
        AniError::StreamServerFailed {
            stage: "stream",
            host: None,
        }
    } else {
        error
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
/// here is evidence of absence. An answered dead end is the
/// episode's own verdict, [`AniError::EpisodeUnavailable`]
/// ([`episode_verdict`]), for every caller alike. A timeout the
/// provider stamps no attempt on ([`Provider::last_attempt_at`]) is
/// the one failure the verdict does not rename: no host failed.
pub async fn resolve_episode<P: Provider + ?Sized>(
    client: &P,
    picked: &PickedShow,
    episode: &str,
    mode: &str,
    quality: &str,
) -> std::result::Result<ResolvedEpisode, NativeError> {
    let dead_end = |error: AniError| NativeError {
        error: match error {
            // A timeout the provider stamps no attempt on is its
            // walk's bound cutting a server off while the gate still
            // held the request: the request the window cut off
            // reached no host, so no host failed. It keeps its name
            // — the deadline's shape, which the
            // breaker reads by its stamp — where the stream server's
            // failure would say a host did not serve, and be distress
            // on every priority.
            AniError::Timeout if client.last_attempt_at().is_none() => AniError::Timeout,
            error => episode_verdict(error),
        },
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
            Some(tag) => tag_matches(tag, &target),
            None => tag_matches(&e.number.to_string(), &target),
        })
        .ok_or_else(|| dead_end(AniError::NoResults))?;
    // One step, validated by the provider: the master answered and
    // the quality was selected from it. The quality step is soft only
    // on a served playlist that lacks the height; a failed master or
    // rendition fetch is the episode failing — or, for a provider
    // that lists several servers, that server failing and the next
    // being tried, which is why the provider does the validating.
    let stream = client
        .stream_for(ep.id, mode, quality)
        .await
        .map_err(dead_end)?;
    Ok(ResolvedEpisode {
        master_url: stream.url,
        referer: stream.referer,
        // Bounded here, in the step every caller resolves through —
        // the play resolve and the range download — so the cache
        // row, the session, the handoffs' argv and both downloads
        // share the one bound.
        subtitles: super::play_native_resolve::tracks_within_cap(stream.subtitles),
        slot: ep.number,
        tag: ep.number2.clone(),
    })
}

/// A resolved episode: the playable URL plus the matched row's slot
/// (its position in the provider's listing) and display tag.
#[derive(Debug)]
pub struct ResolvedEpisode {
    /// The validated master-playlist URL.
    pub master_url: String,
    /// The referer the provider's CDN wants on every fetch of it,
    /// when it wants one.
    pub referer: Option<String>,
    /// The sidecar subtitle tracks the provider listed beside it.
    pub subtitles: Vec<SubtitleTrack>,
    /// The row's integer `number` — the provider's own position for
    /// it.
    pub slot: u32,
    /// The row's display tag, when it carries one.
    pub tag: Option<String>,
}

/// Whether two episode tags name the same episode. The provider can
/// file a tag in a non-canonical string form ("041", "3.50") that
/// the offset, cap, and extras all read numerically — the click must
/// match the same identity. Two parses of numerically identical
/// decimal strings round to the same f64, so equality is exact
/// here; genuinely nonnumeric tags keep byte equality.
pub(super) fn tag_matches(tag: &str, target: &str) -> bool {
    if tag == target {
        return true;
    }
    match (tag.parse::<f64>(), target.parse::<f64>()) {
        (Ok(a), Ok(b)) => a.total_cmp(&b) == std::cmp::Ordering::Equal,
        _ => false,
    }
}

#[cfg(test)]
#[path = "play_native_episode_test.rs"]
mod tests;
