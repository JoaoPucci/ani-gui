//! One budget for every request this app sends AniList.
//!
//! AniList limits the API per client — at present 30 requests a
//! minute while it runs degraded, 90 otherwise — and answers a client
//! past it with a 429 and a one-minute timeout, during which every
//! further request is refused too and the timeout runs on. Nothing in
//! the app owns that budget on its own: trending, banners, episode
//! thumbnails, airing, the id bridges and the signed-in account's
//! reads and writes all spend it, and a cold home page asks for
//! several of them per card at once. So every transport — the public
//! GraphQL POST, the `streamingEpisodes` lookup and the authenticated
//! account POST — sends through the gate kept here for its host, and
//! the gate does two things:
//!
//! - **Paces.** No more than [`Pace::capacity`] requests leave in any
//!   [`Pace::window`], [`Pace::min_gap`] apart, so a burst queues
//!   under the limit instead of tripping it. Background fills —
//!   thumbnails, banners, airing seeds — may use all but
//!   [`Pace::reserve`] of the window, which is held for what the user
//!   is looking at.
//! - **Cools down.** A 429 (or a 503 that says when to come back)
//!   records "not before T", T being now plus the advertised wait or
//!   [`Pace::default_cool_down`] when none came; so does a response
//!   saying no requests remain in the window. Until T nothing is sent.
//!
//! A caller waits for its turn only within its class's budget
//! ([`Pace::interactive_wait`], [`Pace::background_wait`]); past it,
//! it gets [`AniError::RateLimited`] with the wait left, and no
//! request is spent. A 429 that arrives anyway is retried once when
//! its window ends inside the budget, and is otherwise returned as
//! the rate limit it is.
//!
//! Whether AniList counts the public and the authenticated requests
//! in one budget is not documented; its limit is stated per client,
//! and the 429s observed while signed in arrived for both alike. They
//! share one gate: if the budgets are separate, sharing costs some
//! headroom; if they are one, keeping two would spend it twice.
//!
//! The gate is chosen by the request's host. AniList's own host has
//! one for the life of the process. Any other host — a test's mock
//! server — gets a fresh gate per call unless a test shares one
//! ([`share_for_tests`]), so tests that never asked for pacing do not
//! pace each other through a reused mock port.

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::{Arc, LazyLock, Mutex};
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
pub(crate) const ANILIST: Pace = Pace {
    window: Duration::from_secs(60),
    capacity: 25,
    reserve: 5,
    min_gap: Duration::from_millis(250),
    default_cool_down: Duration::from_secs(60),
    interactive_wait: Duration::from_secs(3),
    background_wait: Duration::from_secs(2),
};

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

/// What a gate remembers: when its recent requests left, and the
/// instant before which nothing may.
#[derive(Debug, Default)]
pub(crate) struct Ledger {
    sent: VecDeque<Instant>,
    not_before: Option<Instant>,
}

impl Ledger {
    /// The earliest instant at or after `now` a request of `class` may
    /// leave: past the cool-down, [`Pace::min_gap`] after the last
    /// request, and late enough that the window ending then holds
    /// fewer requests than the class may send in it.
    pub(crate) fn ready_at(&self, pace: &Pace, class: Class, now: Instant) -> Instant {
        let mut at = now;
        if let Some(not_before) = self.not_before {
            at = at.max(not_before);
        }
        if let Some(&last) = self.sent.back() {
            at = at.max(last + pace.min_gap);
        }
        let cap = pace.capacity_for(class);
        if self.sent.len() >= cap {
            at = at.max(self.sent[self.sent.len() - cap] + pace.window);
        }
        at
    }

    /// A request left at `at`. Only the last `capacity` sends can bound
    /// a later one, so older ones are dropped.
    pub(crate) fn record_send(&mut self, pace: &Pace, at: Instant) {
        self.sent.push_back(at);
        while self.sent.len() > pace.capacity.max(1) {
            self.sent.pop_front();
        }
    }

    /// The request that left at `sent` was answered (or failed) at
    /// `done`: it is counted from then on. AniList counts a request
    /// when it arrives, which is after it left by however long the
    /// connection took; counting it from its answer keeps the window
    /// this side ends no earlier than the one AniList keeps.
    pub(crate) fn settle(&mut self, sent: Instant, done: Instant) {
        let Some(i) = self.sent.iter().position(|&s| s == sent) else {
            return;
        };
        self.sent.remove(i);
        let at = self.sent.partition_point(|&s| s <= done);
        self.sent.insert(at, done.max(sent));
    }

    /// Nothing may leave before `until`. A cool-down already running
    /// past it stands.
    pub(crate) fn cool_down(&mut self, until: Instant) {
        self.not_before = Some(self.not_before.map_or(until, |nb| nb.max(until)));
    }
}

struct Gate {
    pace: Pace,
    ledger: Mutex<Ledger>,
}

impl Gate {
    fn new(pace: Pace) -> Self {
        Self {
            pace,
            ledger: Mutex::new(Ledger::default()),
        }
    }

    fn ledger(&self) -> std::sync::MutexGuard<'_, Ledger> {
        self.ledger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Wait for `class`'s turn and take it, or refuse when the turn
    /// comes after `deadline`. Nothing is recorded until the turn is
    /// taken, so a caller dropped while it waits costs nothing.
    /// Returns the instant the turn was taken, for [`Ledger::settle`].
    async fn admit(&self, class: Class, deadline: Instant) -> Result<Instant> {
        loop {
            let now = Instant::now();
            let at = {
                let mut ledger = self.ledger();
                let at = ledger.ready_at(&self.pace, class, now);
                if at <= now {
                    ledger.record_send(&self.pace, now);
                    return Ok(now);
                }
                at
            };
            if at > deadline {
                return Err(not_sent(at - now, class));
            }
            tokio::time::sleep_until(at).await;
        }
    }
}

/// The rate limit a caller gets for a request the gate kept back.
fn not_sent(wait: Duration, class: Class) -> AniError {
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
fn stay_away(resp: &reqwest::Response, pace: &Pace) -> Option<Duration> {
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

fn no_requests_left(resp: &reqwest::Response) -> bool {
    resp.headers()
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        == Some(0)
}

static SHARED: LazyLock<Mutex<HashMap<String, Arc<Gate>>>> = LazyLock::new(|| {
    let mut gates = HashMap::new();
    gates.insert(
        authority(crate::meta::anilist::ANILIST_API),
        Arc::new(Gate::new(ANILIST)),
    );
    Mutex::new(gates)
});

fn shared() -> std::sync::MutexGuard<'static, HashMap<String, Arc<Gate>>> {
    SHARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// `host[:port]` of `url`, the key a gate is kept under.
fn authority(url: &str) -> String {
    url::Url::parse(url).map_or_else(
        |_| url.to_owned(),
        |u| match (u.host_str(), u.port()) {
            (Some(h), Some(p)) => format!("{h}:{p}"),
            (Some(h), None) => h.to_owned(),
            _ => url.to_owned(),
        },
    )
}

fn gate_for(url: &str) -> Arc<Gate> {
    shared()
        .get(&authority(url))
        .cloned()
        .unwrap_or_else(|| Arc::new(Gate::new(ANILIST)))
}

/// Send one AniList request through its host's gate. `send` builds and
/// sends it; it is called once, or twice when a refusal's window ends
/// inside the caller's budget. The response is returned as it came —
/// a 429 included — for the transport's own status mapping.
///
/// # Errors
/// [`AniError::RateLimited`] when the caller's turn would come past its
/// budget (nothing was sent); [`AniError::Network`] when sending
/// failed.
pub(crate) async fn send<F, Fut>(url: &str, send: F) -> Result<reqwest::Response>
where
    F: Fn() -> Fut,
    Fut: Future<Output = reqwest::Result<reqwest::Response>>,
{
    let gate = gate_for(url);
    let class = current_class();
    let deadline = Instant::now() + gate.pace.wait_for(class);
    let mut retried = false;
    loop {
        let sent = gate.admit(class, deadline).await?;
        let resp = send().await;
        gate.ledger().settle(sent, Instant::now());
        let resp = resp.map_err(|_| AniError::Network)?;
        let Some(wait) = stay_away(&resp, &gate.pace) else {
            return Ok(resp);
        };
        let until = Instant::now() + wait;
        gate.ledger().cool_down(until);
        let refused = !resp.status().is_success();
        if !refused || retried || until > deadline {
            return Ok(resp);
        }
        tracing::warn!(
            provider = "anilist",
            status = resp.status().as_u16(),
            wait_ms = u64::try_from(wait.as_millis()).unwrap_or(u64::MAX),
            "tracker refused; one retry once its window ends"
        );
        retried = true;
    }
}

/// Share one gate under `url`'s host with `pace` until the guard drops.
#[cfg(test)]
pub(crate) fn share_for_tests(url: &str, pace: Pace) -> SharedForTests {
    let key = authority(url);
    shared().insert(key.clone(), Arc::new(Gate::new(pace)));
    SharedForTests(key)
}

/// Removes a test's shared gate when dropped.
#[cfg(test)]
pub(crate) struct SharedForTests(String);

#[cfg(test)]
impl Drop for SharedForTests {
    fn drop(&mut self) {
        shared().remove(&self.0);
    }
}

#[cfg(test)]
#[path = "anilist_gate_prop_test.rs"]
mod prop_tests;

#[cfg(test)]
#[path = "anilist_gate_test.rs"]
mod tests;
