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
//! download charges the subtitle tracks it stages beside its transfer
//! and, while playback is live, its own fetches through the proxy,
//! since the host counts them all against the one address — as
//! background traffic, which takes a token only while no one waits for
//! one until it has waited [`BACKGROUND_PATIENCE`], when it is served in
//! turn with the player, and which never takes the last
//! [`BACKGROUND_RESERVE`] of the bucket, so the player's next requests
//! find them there. The bucket is per
//! host: a download from a different host than the player's has a
//! bucket of its own, and the two meet only if the host counts them
//! together. Of the app's own fetches, not charged: a cached
//! resolution's liveness check — a ping and a read of each track, at
//! most a track cap's worth at once, before the player starts, under a
//! deadline of seconds that waiting for tokens would spend — and what
//! runs outside the app's client, the resolver's fetch of a playlist
//! through the impersonating transport and the download tools when
//! nothing plays, or after a run through the proxy failed. An external
//! player the app hands a stream to fetches from the host on its own,
//! outside the app entirely.

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

/// Tokens background traffic leaves in the bucket: the player's next
/// request and a seek's few are served from them at once, however much
/// background traffic has been taking what the player was not using.
pub(crate) const BACKGROUND_RESERVE: u32 = 5;

/// How long background traffic yields to the player before it joins
/// the host's line and is served in turn. A player filling its buffer
/// keeps a request waiting at all times, for a minute after a start or
/// a seek; without a limit a download beside it would take nothing for
/// all that time.
pub(crate) const BACKGROUND_PATIENCE: Duration = Duration::from_millis(4500);

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
#[cfg(test)]
pub(crate) fn take(
    bucket: &mut Bucket,
    now: Instant,
    burst: u32,
    refill: Duration,
) -> Option<Duration> {
    take_leaving(bucket, now, burst, refill, 0)
}

/// [`take`], leaving `reserve` tokens behind: a token is taken only
/// while more than the reserve is in the bucket, and otherwise the
/// wait is until there is.
pub(crate) fn take_leaving(
    bucket: &mut Bucket,
    now: Instant,
    burst: u32,
    refill: Duration,
    reserve: u32,
) -> Option<Duration> {
    // A reserve as large as the burst leaves background traffic
    // nothing: it would wait for good.
    debug_assert!(
        reserve < burst,
        "the reserve {reserve} leaves no room under the burst {burst}"
    );
    let elapsed = now.saturating_duration_since(bucket.refilled_at);
    let refilled = elapsed.as_secs_f64() / refill.as_secs_f64();
    bucket.tokens = (bucket.tokens + refilled).min(f64::from(burst));
    bucket.refilled_at = now;
    let needed = 1.0 + f64::from(reserve);
    if bucket.tokens >= needed {
        bucket.tokens -= 1.0;
        None
    } else {
        // A shade past the exact instant, so a caller who waits this
        // long and asks again is not turned away by rounding.
        Some(refill.mul_f64(needed - bucket.tokens) + Duration::from_millis(1))
    }
}

/// The budgets of every host fetched from, and the line of requests
/// waiting at each.
pub struct HostBudget {
    buckets: Mutex<HashMap<String, Bucket>>,
    lines: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    burst: u32,
    refill: Duration,
    reserve: u32,
}

impl HostBudget {
    #[must_use]
    pub(crate) fn new(burst: u32, refill: Duration) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            lines: Mutex::new(HashMap::new()),
            burst,
            refill,
            reserve: 0,
        }
    }

    /// The budget an app state is built with: the app's constants,
    /// shared by the proxy it builds and the downloads it runs. The app
    /// builds one, so this is one budget per process — the address the
    /// host counts.
    #[must_use]
    pub fn fresh() -> Arc<Self> {
        Arc::new(Self {
            reserve: BACKGROUND_RESERVE,
            ..Self::new(SEGMENT_BURST, SEGMENT_REFILL)
        })
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
            match self.take_for(host, 0) {
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
    /// which within its patience takes no place in the host's line:
    /// it takes a token only while no one is waiting for one, and
    /// otherwise waits a refill and looks again. Whoever is in the
    /// line is served first while the background fetch is within its
    /// patience; with the line empty it waits for its token like any
    /// other. It never takes the budget's reserve
    /// ([`BACKGROUND_RESERVE`] in the app's budget), which is left for
    /// the player's next requests. Once it has waited
    /// [`BACKGROUND_PATIENCE`] it stops yielding and joins the line,
    /// served in turn with the player, so a player filling its buffer
    /// cannot keep it waiting for the whole fill.
    pub(crate) async fn admit_background(&self, host: &str) {
        let line = self.line(host);
        let out_of_patience = Instant::now() + BACKGROUND_PATIENCE;
        loop {
            let wait = match line.try_lock() {
                Ok(_nobody_waiting) => match self.take_for(host, self.reserve) {
                    None => return,
                    Some(wait) => wait,
                },
                Err(_someone_waiting) => self.refill,
            };
            let now = Instant::now();
            if now >= out_of_patience {
                break;
            }
            tokio::time::sleep(wait.min(out_of_patience - now)).await;
        }
        // Waited its patience: it joins the line and is served in turn
        // with the player.
        self.admit(host).await;
    }

    /// The line of requests waiting at `host`.
    fn line(&self, host: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut lines = self.lines.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(lines.entry(host.to_owned()).or_default())
    }

    /// Takes a token for `host` now, leaving `reserve` behind, or says
    /// how long until it can.
    fn take_for(&self, host: &str, reserve: u32) -> Option<Duration> {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let bucket = buckets
            .entry(host.to_owned())
            .or_insert_with(|| Bucket::full(self.burst, now));
        take_leaving(bucket, now, self.burst, self.refill, reserve)
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
