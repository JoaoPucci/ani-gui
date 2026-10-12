//! One reading of a tracker's HTTP 429 for every AniList and MAL
//! transport: the authenticated AniList POST, the public GraphQL POST,
//! MAL's bearer verbs and both token endpoints.
//!
//! A 429 is the tracker refusing this client for a while, not an
//! answer about the request. It becomes [`AniError::RateLimited`]
//! carrying the `Retry-After` the tracker advertised — AniList sends
//! the seconds left in its per-minute window — so the route answers
//! under `error.network.rate_limited` with the wait, and the renderer
//! can wait that long instead of retrying on a schedule of its own.
//! Each one is logged at WARN with the provider, the host and the
//! wait, so the next report of a disabled list button is traceable.

use std::time::Duration;

use crate::error::AniError;

/// The seconds a `Retry-After` header asks for, when it is the
/// delay-seconds form the trackers use. The HTTP-date form, and
/// anything else, reads as no advertised wait.
#[must_use]
pub(crate) fn parse_retry_after(value: Option<&str>) -> Option<u64> {
    value?.trim().parse::<u64>().ok()
}

/// The advertised wait on a response, read as [`parse_retry_after`].
#[must_use]
pub(crate) fn retry_after_secs(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    parse_retry_after(
        headers
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok()),
    )
}

/// How long a bounded retry may sleep for an advertised wait, or
/// `None` when it should not retry at all. No wait advertised sleeps
/// `default`; an advertised wait up to `cap` is honoured as is; one
/// past the cap is not retried — a retry before the window ends is a
/// request spent to be told the same thing, and the caller surfaces
/// the rate limit with its wait at once.
#[must_use]
pub(crate) fn retry_wait_within(
    advertised: Option<u64>,
    cap: Duration,
    default: Duration,
) -> Option<Duration> {
    match advertised {
        None => Some(default),
        Some(secs) => {
            let wait = Duration::from_secs(secs);
            (wait <= cap).then_some(wait)
        }
    }
}

/// The error a 429 from `provider` becomes, logged here so every
/// transport reports the same way.
pub(crate) fn rate_limited(provider: &'static str, resp: &reqwest::Response) -> AniError {
    let retry_after_secs = retry_after_secs(resp.headers());
    tracing::warn!(
        provider,
        host = resp.url().host_str().unwrap_or(""),
        retry_after = ?retry_after_secs,
        "tracker answered 429"
    );
    AniError::RateLimited { retry_after_secs }
}
