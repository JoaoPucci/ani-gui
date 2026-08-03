//! The gate's outcome vocabulary: what an admitted request reports
//! back, and how a typed request result maps into it. Split from the
//! gate machine so the pure classification surface reads (and
//! measures) on its own.

use tokio::time::Duration;

/// Typed result of an admitted request, replacing the bool for
/// callers that can distinguish allanime's in-band rate limit (and
/// its advertised retry hint) from an untyped failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrapeOutcome {
    /// The request succeeded.
    Success,
    /// The request failed for an untyped reason (transport error,
    /// garbage body). Feeds the consecutive-failure breaker.
    Failure,
    /// allanime answered with its in-band rate limit. One such
    /// response opens the pause window immediately — no threshold
    /// counting; the upstream said plainly to go away, and told us
    /// for how long when `retry_after` is `Some`.
    RateLimited {
        /// The advertised wait, when the response carried one.
        retry_after: Option<Duration>,
    },
}
