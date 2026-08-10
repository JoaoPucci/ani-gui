//! The resolution → breaker mapping — split from `play_native_resolve`
//! so each file stays inside the complexity ratchet's per-file bar.

use super::play_native_resolve::{NativeError, NativeResolved};
use crate::error::AniError;
use crate::scraper::gate::ScrapeOutcome;

/// What a resolution outcome means to the scraper breaker. The
/// provider ANSWERING is health whatever the answer was: every
/// `NoResults` arises from an answered verdict — a rejected pool, an
/// absent episode, a sub-only show asked for dub — while weather
/// (transport, refusals, rate limits) is distress. Distinct from
/// `clean_miss`, which additionally decides whether the verdict is
/// persistable absence: a matched show with one episode missing is
/// breaker-healthy but proves nothing about the show's availability.
pub fn breaker_outcome(native: &std::result::Result<NativeResolved, NativeError>) -> ScrapeOutcome {
    match native {
        Ok(_) => ScrapeOutcome::Success,
        Err(ne) if ne.clean_miss => ScrapeOutcome::Success,
        Err(ne) => match ne.error {
            AniError::NoResults => ScrapeOutcome::Success,
            AniError::RateLimited { retry_after_secs } => ScrapeOutcome::RateLimited {
                retry_after: retry_after_secs.map(std::time::Duration::from_secs),
            },
            // anidb rate-limits with the HTTP status, not the
            // in-band payload the typed variant carries: the
            // provider explicitly asked clients to stop, so the
            // hintless rate-limit outcome opens the advertised
            // pause immediately instead of burning the failure
            // threshold on queued background resolutions.
            AniError::Upstream { status: 429 } => ScrapeOutcome::RateLimited { retry_after: None },
            _ => ScrapeOutcome::Failure,
        },
    }
}

#[cfg(test)]
#[path = "play_native_outcome_test.rs"]
mod tests;
