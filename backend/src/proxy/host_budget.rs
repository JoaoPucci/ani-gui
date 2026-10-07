//! The host counts requests per address and refuses the address with
//! 429s once the count runs high. The player fetches segments as fast
//! as the host answers — an episode's worth in the first twenty seconds
//! of pressing play — and in a live run that alone reached the refusal,
//! with a download's requests beside it tipping it sooner. Every fetch
//! the player makes passes through the proxy, so the proxy spaces them:
//! each upstream host has a budget with a burst for startup and seeks
//! and a steady refill after it, and every fetch the proxy makes to the
//! host on the player's behalf — playlists, segments, mp4 ranges, and
//! the subtitle tracks the player loads, which wait behind the media —
//! is charged to it. hls.js loads one segment at a time and the player
//! allows it ten seconds for a first byte, so a
//! wait here of a second or so is absorbed, and the player still
//! buffers as far ahead as it likes — over minutes rather than
//! seconds; while it is still filling that buffer, a seek past it may
//! take a few seconds longer than it otherwise would. The budget is
//! per host and per app: the proxy charges every fetch it makes on the
//! player's behalf, hop by hop where a redirect sends it on, and a
//! download charges the subtitle tracks it stages beside its transfer,
//! since the host counts both against the one address — as background
//! traffic, which takes a token only while no one waits for one. Of the app's
//! own fetches, not charged: a cached resolution's liveness check — a
//! ping and a read of each track, at most a track cap's worth at once,
//! before the player starts, under a deadline of seconds that waiting
//! for tokens would spend — and what runs outside the app's client,
//! the resolver's fetch of a playlist through the impersonating
//! transport and the download tools, which the lane paces instead. An
//! external player the app hands a stream to fetches from the host on
//! its own, outside the app entirely.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::time::Instant;
use url::Url;

/// Requests a host answers without waiting: the playlists, the first
/// segments, a seek's worth.
pub(crate) const SEGMENT_BURST: u32 = 20;

/// The steady rate once the burst is spent: one request per this
/// interval, forty a minute. A segment plays for about five seconds,
/// so the buffer still grows three times faster than playback drains
/// it.
pub(crate) const SEGMENT_REFILL: Duration = Duration::from_millis(1500);

/// One host's budget: the tokens on hand and when they were last
/// topped up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Bucket {
    tokens: f64,
    refilled_at: Instant,
}

impl Bucket {
    /// A full bucket at `now`.
    #[must_use]
    pub(crate) fn full(burst: u32, now: Instant) -> Self {
        Self {
            tokens: f64::from(burst),
            refilled_at: now,
        }
    }

    #[cfg(test)]
    pub(crate) fn tokens(&self) -> f64 {
        self.tokens
    }
}

/// Takes one token if the bucket has one at `now`; otherwise says how
/// long until it does. Pure over its inputs: the bucket is topped up by
/// the time since it last was, one token per `refill`, capped at
/// `burst`.
pub(crate) fn take(
    bucket: &mut Bucket,
    now: Instant,
    burst: u32,
    refill: Duration,
) -> Option<Duration> {
    let elapsed = now.saturating_duration_since(bucket.refilled_at);
    let refilled = elapsed.as_secs_f64() / refill.as_secs_f64();
    bucket.tokens = (bucket.tokens + refilled).min(f64::from(burst));
    bucket.refilled_at = now;
    if bucket.tokens >= 1.0 {
        bucket.tokens -= 1.0;
        None
    } else {
        // A shade past the exact instant, so a caller who waits this
        // long and asks again is not turned away by rounding.
        Some(refill.mul_f64(1.0 - bucket.tokens) + Duration::from_millis(1))
    }
}

/// Spends `n` tokens from the bucket at `now` without waiting for
/// them: what reached the host past the budget. Topped up first, as a
/// take is; never below empty.
pub(crate) fn spend_from(bucket: &mut Bucket, now: Instant, burst: u32, refill: Duration, n: u32) {
    let elapsed = now.saturating_duration_since(bucket.refilled_at);
    let refilled = elapsed.as_secs_f64() / refill.as_secs_f64();
    bucket.tokens = (bucket.tokens + refilled).min(f64::from(burst));
    bucket.refilled_at = now;
    bucket.tokens = (bucket.tokens - f64::from(n)).max(0.0);
}

/// Tops the bucket up at `now`, as a take does, and holds it to `most`
/// tokens.
pub(crate) fn hold_to(bucket: &mut Bucket, now: Instant, burst: u32, refill: Duration, most: u32) {
    let elapsed = now.saturating_duration_since(bucket.refilled_at);
    let refilled = elapsed.as_secs_f64() / refill.as_secs_f64();
    bucket.tokens = (bucket.tokens + refilled)
        .min(f64::from(burst))
        .min(f64::from(most));
    bucket.refilled_at = now;
}

/// The budgets of every host fetched from, the line of requests
/// waiting at each, and the unpaced downloads running against each.
pub struct HostBudget {
    buckets: Mutex<HashMap<String, Bucket>>,
    lines: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Requests the unpaced downloads running against each host may
    /// have in flight.
    unpaced: Mutex<HashMap<String, u32>>,
    burst: u32,
    refill: Duration,
}

/// An unpaced download running against a host: while it runs, the
/// host's tokens are held to what its requests in flight leave of the
/// burst; when it ends, those requests are spent.
pub(crate) struct UnpacedRun<'a> {
    budget: &'a HostBudget,
    host: String,
    in_flight: u32,
}

impl Drop for UnpacedRun<'_> {
    fn drop(&mut self) {
        {
            let mut unpaced = self
                .budget
                .unpaced
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(n) = unpaced.get_mut(&self.host) {
                *n = n.saturating_sub(self.in_flight);
                if *n == 0 {
                    unpaced.remove(&self.host);
                }
            }
        }
        self.budget.spend(&self.host, self.in_flight);
    }
}

impl HostBudget {
    #[must_use]
    pub(crate) fn new(burst: u32, refill: Duration) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            lines: Mutex::new(HashMap::new()),
            unpaced: Mutex::new(HashMap::new()),
            burst,
            refill,
        }
    }

    /// The budget an app state is built with: the app's constants,
    /// shared by the proxy it builds and the downloads it runs. The app
    /// builds one, so this is one budget per process — the address the
    /// host counts.
    #[must_use]
    pub fn fresh() -> Arc<Self> {
        Arc::new(Self::new(SEGMENT_BURST, SEGMENT_REFILL))
    }

    /// The tokens `host` has on hand as of its last take, for a test
    /// to see what a fetch spent; `None` for a host never fetched from.
    #[cfg(test)]
    pub(crate) fn on_hand(&self, host: &str) -> Option<f64> {
        self.buckets
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(host)
            .map(Bucket::tokens)
    }

    /// A token for `host`, waiting for one while the burst is spent.
    /// Waiters are served in the order they arrived: each takes its
    /// place in the host's line and keeps it until it has its token,
    /// so a request arriving as a token matures does not take it from
    /// one that has waited for it, and a few requests arriving at once
    /// cannot keep taking the tokens ahead of the one that has waited
    /// longest.
    pub(crate) async fn admit(&self, host: &str) {
        let line = self.line(host);
        let _place = line.lock().await;
        loop {
            match self.take_for(host) {
                None => return,
                Some(wait) => {
                    tracing::debug!(
                        host,
                        wait_ms = wait.as_millis(),
                        "budget: pacing a fetch to the host's budget",
                    );
                    tokio::time::sleep(wait).await;
                }
            }
        }
    }

    /// A token for `host` for background traffic — subtitle tracks,
    /// the player's and a download's, which playback can wait for —
    /// which never takes a place in the host's line:
    /// it takes a token only while no one is waiting for one, and
    /// otherwise waits a refill and looks again. Whoever is in the
    /// line is served first, however long the background fetch has
    /// been waiting; with the line empty it waits for its token like
    /// any other.
    pub(crate) async fn admit_background(&self, host: &str) {
        let line = self.line(host);
        loop {
            let wait = match line.try_lock() {
                Ok(_nobody_waiting) => match self.take_for(host) {
                    None => return,
                    Some(wait) => wait,
                },
                Err(_someone_waiting) => self.refill,
            };
            tokio::time::sleep(wait).await;
        }
    }

    /// An unpaced download starts against `host`, with up to
    /// `in_flight` requests at a time that pass the budget: the host's
    /// tokens are held to what they leave of the burst until it ends,
    /// and spent by them when it does ([`UnpacedRun`]).
    pub(crate) fn unpaced_run(&self, host: &str, in_flight: u32) -> UnpacedRun<'_> {
        *self
            .unpaced
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(host.to_owned())
            .or_default() += in_flight;
        UnpacedRun {
            budget: self,
            host: host.to_owned(),
            in_flight,
        }
    }

    /// Spends `n` tokens of `host`'s budget without waiting for them:
    /// requests the app sent past the budget, which the host counts.
    pub(crate) fn spend(&self, host: &str, n: u32) {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let bucket = buckets
            .entry(host.to_owned())
            .or_insert_with(|| Bucket::full(self.burst, now));
        spend_from(bucket, now, self.burst, self.refill, n);
    }

    /// The line of requests waiting at `host`.
    fn line(&self, host: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut lines = self.lines.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(lines.entry(host.to_owned()).or_default())
    }

    /// Takes a token for `host` now, or says how long until one.
    fn take_for(&self, host: &str) -> Option<Duration> {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let bucket = buckets
            .entry(host.to_owned())
            .or_insert_with(|| Bucket::full(self.burst, now));
        self.hold_to_unpaced(host, bucket, now);
        take(bucket, now, self.burst, self.refill)
    }

    /// While unpaced downloads run against `host`, holds its tokens to
    /// what their requests in flight leave of the burst — never below
    /// one, since the player's first request is what tells them that
    /// playback started.
    fn hold_to_unpaced(&self, host: &str, bucket: &mut Bucket, now: Instant) {
        let in_flight = self
            .unpaced
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(host)
            .copied()
            .unwrap_or(0);
        if in_flight > 0 {
            let most = self.burst.saturating_sub(in_flight).max(1);
            hold_to(bucket, now, self.burst, self.refill, most);
        }
    }
}

/// The key a URL's host is budgeted under: host and port, so two
/// servers on one machine — the test servers, for one — are two
/// budgets.
#[must_use]
pub(crate) fn host_key(url: &Url) -> String {
    format!(
        "{}:{}",
        url.host_str().unwrap_or(""),
        url.port_or_known_default().unwrap_or(0)
    )
}

#[cfg(test)]
#[path = "host_budget_test.rs"]
mod tests;
