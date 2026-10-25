//! How an answer — or a request kept back — reads to the gate: the
//! rate limit a caller gets for a request not sent, how long an answer
//! tells this client to stay away, and the key a gate is kept under.
//! Split from [`super`] so each file stays inside the CRAP gate's
//! per-file bar.

use std::time::Duration;

use super::pace::{Class, Pace};
use crate::error::AniError;

/// The rate limit a caller gets for a request the gate kept back.
pub(super) fn not_sent(wait: Duration, class: Class) -> AniError {
    let retry_after_secs = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
    tracing::info!(
        provider = "anilist",
        ?class,
        retry_after_secs,
        "anilist budget spent; request not sent"
    );
    AniError::RateLimited {
        retry_after_secs: Some(retry_after_secs),
    }
}

/// How long `resp` tells this client to stay away, if it does: a 429
/// for its `Retry-After` (or `default`), a 503 only when it names a
/// wait, and any answer whose `X-RateLimit-Remaining` is zero for a
/// whole window — the next request would be the one refused.
pub(super) fn stay_away(resp: &reqwest::Response, pace: &Pace) -> Option<Duration> {
    let advertised = crate::meta::rate_limit::retry_after_secs(resp.headers());
    match resp.status() {
        reqwest::StatusCode::TOO_MANY_REQUESTS => Some(
            advertised
                .map(Duration::from_secs)
                .unwrap_or(pace.default_cool_down),
        ),
        reqwest::StatusCode::SERVICE_UNAVAILABLE if advertised.is_some() => {
            advertised.map(Duration::from_secs)
        }
        _ => no_requests_left(resp).then_some(pace.window),
    }
}

pub(super) fn no_requests_left(resp: &reqwest::Response) -> bool {
    resp.headers()
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        == Some(0)
}

/// `host:port` of `url`, the key a gate is kept under; `url` itself
/// when it names no host.
pub(super) fn authority(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| Some(format!("{}:{}", u.host_str()?, u.port_or_known_default()?)))
        .unwrap_or_else(|| url.to_owned())
}
