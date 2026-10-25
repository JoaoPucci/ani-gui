//! The one way every AniList request is sent. Today it carries the
//! public transport's bounded retry; the budget AniList's rate limit
//! asks for is kept here.

use std::future::Future;
use std::time::Duration;

use tokio::time::Instant;

use crate::error::{AniError, Result};

/// Who is waiting for an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    /// What the user is looking at or did: the default.
    Interactive,
    /// A fill the page renders without: thumbnails, banners, the
    /// airing seeds a warm writes.
    Background,
}

tokio::task_local! {
    static CLASS: Class;
}

/// Run `f` with every AniList request it sends counted as
/// [`Class::Background`].
pub(crate) async fn in_background<F: Future>(f: F) -> F::Output {
    CLASS.scope(Class::Background, f).await
}

fn current_class() -> Class {
    CLASS.try_with(|c| *c).unwrap_or(Class::Interactive)
}

/// How a gate spends its host's budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct Pace {
    /// The span the capacity is counted over.
    pub window: Duration,
    /// Requests allowed to leave in any one window.
    pub capacity: usize,
    /// Of the capacity, what background fills may not use.
    pub reserve: usize,
    /// The least time between two requests, against a burst limiter.
    pub min_gap: Duration,
    /// The cool-down a refusal without a usable `Retry-After` gets.
    pub default_cool_down: Duration,
    /// How long an interactive caller may wait for its turn.
    pub interactive_wait: Duration,
    /// How long a background caller may wait for its turn.
    pub background_wait: Duration,
}

/// AniList's budget: 25 a minute against its degraded 30, a quarter
/// second apart, five of each minute held for the user. A refusal
/// with no wait is AniList's documented one-minute timeout. An
/// interactive read waits up to three seconds — the retry cap the
/// public transport has always had — and a background fill two, the
/// budget the episode-thumbnail backfill already runs under.
#[allow(dead_code)]
pub(crate) const ANILIST: Pace = Pace {
    window: Duration::from_secs(60),
    capacity: 25,
    reserve: 5,
    min_gap: Duration::from_millis(250),
    default_cool_down: Duration::from_secs(60),
    interactive_wait: Duration::from_secs(3),
    background_wait: Duration::from_secs(2),
};

#[allow(dead_code)]
impl Pace {
    fn wait_for(&self, class: Class) -> Duration {
        match class {
            Class::Interactive => self.interactive_wait,
            Class::Background => self.background_wait,
        }
    }

    fn capacity_for(&self, class: Class) -> usize {
        match class {
            Class::Interactive => self.capacity,
            Class::Background => self.capacity.saturating_sub(self.reserve),
        }
        .max(1)
    }
}

/// What a gate remembers. Nothing yet: every request leaves at once.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub(crate) struct Ledger;

#[allow(dead_code)]
impl Ledger {
    /// The earliest instant at or after `now` a request may leave.
    pub(crate) fn ready_at(&self, _pace: &Pace, _class: Class, now: Instant) -> Instant {
        now
    }

    /// A request left at `at`.
    pub(crate) fn record_send(&mut self, _pace: &Pace, _at: Instant) {}

    /// Nothing may leave before `until`.
    pub(crate) fn cool_down(&mut self, _until: Instant) {}
}

/// The longest a request sleeps before its one retry, and the wait it
/// assumes when a 429 carries no `Retry-After`.
const RETRY_CAP: Duration = Duration::from_secs(3);
const RETRY_DEFAULT: Duration = Duration::from_secs(1);

/// Send one AniList request. `send` builds and sends it; a 429 whose
/// advertised wait is within [`RETRY_CAP`] is retried once after it.
/// The response is returned as it came — a 429 included — for the
/// transport's own status mapping.
///
/// # Errors
/// [`AniError::Network`] when sending failed.
pub(crate) async fn send<F, Fut>(_url: &str, send: F) -> Result<reqwest::Response>
where
    F: Fn() -> Fut,
    Fut: Future<Output = reqwest::Result<reqwest::Response>>,
{
    let _ = current_class();
    let mut resp = send().await.map_err(|_| AniError::Network)?;
    if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let advertised = crate::meta::rate_limit::retry_after_secs(resp.headers());
        if let Some(wait) =
            crate::meta::rate_limit::retry_wait_within(advertised, RETRY_CAP, RETRY_DEFAULT)
        {
            tokio::time::sleep(wait).await;
            resp = send().await.map_err(|_| AniError::Network)?;
        }
    }
    Ok(resp)
}

/// Share one gate under `url`'s host with `pace` until the guard drops.
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn share_for_tests(url: &str, _pace: Pace) -> SharedForTests {
    SharedForTests(url.to_owned())
}

/// Removes a test's shared gate when dropped.
#[cfg(test)]
#[allow(dead_code)]
pub(crate) struct SharedForTests(String);

#[cfg(test)]
impl Drop for SharedForTests {
    fn drop(&mut self) {}
}
