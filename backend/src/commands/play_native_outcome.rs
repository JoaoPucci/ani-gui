//! The resolution → breaker mapping — split from `play_native_resolve`
//! so each file stays inside the complexity ratchet's per-file bar.

use super::play_native_resolve::NativeError;
use crate::error::AniError;
use crate::scraper::gate::{ScrapeOutcome, ScrapePriority};

/// What a resolution outcome means to the scraper breaker. The
/// provider ANSWERING is health whatever the answer was: every
/// `NoResults` arises from an answered verdict — a rejected pool, an
/// absent episode, a sub-only show asked for dub — while weather
/// (transport, refusals, rate limits) is distress. A gate refusal is
/// neither: it is the gate's OWN answer, so it produces no outcome
/// at all — recording it as failure would let background warmups
/// keep extending the open breaker's cooldown without a single
/// provider request. Distinct from `clean_miss`, which additionally
/// decides whether the verdict is persistable absence: a matched
/// show with one episode missing is breaker-healthy but proves
/// nothing about the show's availability.
pub fn breaker_outcome<T>(
    priority: ScrapePriority,
    native: &std::result::Result<T, NativeError>,
) -> Option<ScrapeOutcome> {
    match native {
        Ok(_) => Some(ScrapeOutcome::Success),
        Err(ne) if ne.clean_miss => Some(ScrapeOutcome::Success),
        Err(ne) => match ne.error {
            AniError::GateRefused => None,
            // A Timeout here is the whole-resolve deadline (the
            // walk turns per-fetch stalls into Network). For a paced
            // background resolve that duration includes the gate's
            // own queueing — a page warm's dozen prefetch chains can
            // burn the deadline on pacing alone — so the elapse is
            // ambiguous evidence and records nothing. Interactive
            // resolves are never paced: their elapse is a real
            // stall and falls through to the failure arm.
            AniError::Timeout if priority == ScrapePriority::Background => None,
            AniError::NoResults => Some(ScrapeOutcome::Success),
            AniError::RateLimited { retry_after_secs } => Some(ScrapeOutcome::RateLimited {
                retry_after: retry_after_secs.map(std::time::Duration::from_secs),
            }),
            // anidb rate-limits with the HTTP status, not the
            // in-band payload the typed variant carries: the
            // provider explicitly asked clients to stop, so the
            // hintless rate-limit outcome opens the advertised
            // pause immediately instead of burning the failure
            // threshold on queued background resolutions.
            AniError::Upstream { status: 429 } => {
                Some(ScrapeOutcome::RateLimited { retry_after: None })
            }
            // Any other non-block status: the provider answered, the
            // answer was "not here" — a dead source at the episode
            // step, the twin of the picker's stale-slug 404. Health.
            // Block-shaped statuses (403, 5xx) fail the guard and
            // fall through to distress.
            AniError::Upstream { .. } if !ne.error.is_provider_block() => {
                Some(ScrapeOutcome::Success)
            }
            _ => Some(ScrapeOutcome::Failure),
        },
    }
}

#[cfg(test)]
#[path = "play_native_outcome_test.rs"]
mod tests;
