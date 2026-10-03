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
//! background traffic, one request at a time, which while no one waits
//! takes a token only above the last [`BACKGROUND_RESERVE`] of the
//! bucket, so the player's next requests find them there, and while the
//! player waits takes every other token with it, so a player filling
//! its buffer neither starves it nor is starved. The bucket is per
//! host: a download from a different host than the player's has a
//! bucket of its own, and the two meet only if the host counts them
//! together. What reaches the host past the budget is charged to it
//! too: a cached resolution's liveness check spends a token per request
//! without waiting — it runs before the player starts, under a deadline
//! of seconds — and while a download tool runs, every host's tokens are
//! held to what the tool's requests in flight leave of the burst, never
//! below one: its fragments go wherever its playlists send them, which
//! the app does not see. When it ends, every host is left where the
//! hold left it, one not yet fetched from included ([`UnpacedRun`]).
//! Not charged: the resolver's fetches of a playlist through the
//! impersonating transport, one or two requests ahead of the player's
//! own, and an external player the app hands a stream to, which fetches
//! from the host on its own, outside the app entirely.

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
/// it, and over one and a half times as fast while background traffic
/// takes every other token.
pub(crate) const SEGMENT_REFILL: Duration = Duration::from_millis(1500);

/// Tokens background traffic leaves in the bucket: the player's next
/// request and a seek's few are served from them at once, however much
/// background traffic has been taking what the player was not using.
pub(crate) const BACKGROUND_RESERVE: u32 = 5;

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

/// Whose turn the next contended token at a host is. A player filling
/// its buffer keeps a request waiting at all times, for a minute after
/// a start or a seek; background traffic that took only the tokens no
/// one waited for took nothing for all that time. So while background
/// traffic waits beside a waiting player, a token the player had to
/// wait for gives the next one to the background request.
#[derive(Debug, Default)]
struct Turn {
    /// A background request is waiting for a token.
    background_waiting: bool,
    /// The player has been served a token it waited for since the
    /// waiting background request arrived, so the next is the
    /// background request's.
    background_owed: bool,
}

/// One host's tokens and turn.
#[derive(Debug)]
struct HostState {
    bucket: Bucket,
    turn: Turn,
}

/// Clears the waiting background request's turn when it is served or
/// given up, so a request dropped while it waits leaves no turn behind
/// it for the player to yield to.
struct BackgroundWaiting<'a> {
    budget: &'a HostBudget,
    host: &'a str,
}

impl Drop for BackgroundWaiting<'_> {
    fn drop(&mut self) {
        self.budget
            .with_state(self.host, |state| state.turn = Turn::default());
    }
}

/// The budgets of every host fetched from, the line of requests
/// waiting at each, and the lane background requests take their turn
/// from one at a time.
pub struct HostBudget {
    states: Mutex<HashMap<String, HostState>>,
    lines: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    lanes: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// The unpaced downloads running, and where they left a host not
    /// yet fetched from.
    unpaced: Mutex<Unpaced>,
    burst: u32,
    refill: Duration,
    reserve: u32,
}

/// The unpaced downloads running — the requests they may have in
/// flight between them — and, once one has ended, where that left a
/// host the player has not fetched from yet: a bucket held as every
/// host's was, refilling as theirs do.
#[derive(Debug, Default)]
struct Unpaced {
    in_flight: u32,
    untouched: Option<Bucket>,
}

/// An unpaced download running: while it runs, every host's tokens are
/// held to what its requests in flight leave of the burst — its
/// fragments go wherever its playlists send them, which the app does
/// not see — and when it ends every host is left where the hold left
/// it, a host not yet fetched from starting there too.
pub(crate) struct UnpacedRun<'a> {
    budget: &'a HostBudget,
    in_flight: u32,
}

impl Drop for UnpacedRun<'_> {
    fn drop(&mut self) {
        let now = Instant::now();
        // Hosts first, then the run count, the order admissions take
        // them in: no admission sees the run ended before every bucket
        // is held.
        let mut states = self.budget.states.lock().unwrap_or_else(|e| e.into_inner());
        // Every host was held to what all the runs going leave, this
        // one's requests and any other's, so that is where it ends.
        let most = {
            let mut unpaced = self
                .budget
                .unpaced
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let most = self.budget.held_to(unpaced.in_flight);
            unpaced.in_flight = unpaced.in_flight.saturating_sub(self.in_flight);
            let untouched = unpaced
                .untouched
                .get_or_insert_with(|| Bucket::full(self.budget.burst, now));
            hold_to(untouched, now, self.budget.burst, self.budget.refill, most);
            most
        };
        for state in states.values_mut() {
            hold_to(
                &mut state.bucket,
                now,
                self.budget.burst,
                self.budget.refill,
                most,
            );
        }
    }
}

impl HostBudget {
    #[must_use]
    pub(crate) fn new(burst: u32, refill: Duration) -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
            lines: Mutex::new(HashMap::new()),
            lanes: Mutex::new(HashMap::new()),
            unpaced: Mutex::new(Unpaced::default()),
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
        self.states
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(host)
            .map(|state| state.bucket.tokens())
    }

    /// A token for `host`, waiting for one while the burst is spent.
    /// Waiters are served in the order they arrived: each takes its
    /// place in the host's line and keeps it until it has its token,
    /// so a request arriving as a token matures does not take it from
    /// one that has waited for it, and a few requests arriving at once
    /// cannot keep taking the tokens ahead of the one that has waited
    /// longest. A token it had to wait for while background traffic
    /// waits gives the next one to the background request, and while
    /// that one is owed the line yields it.
    pub(crate) async fn admit(&self, host: &str) {
        let line = self.line(host);
        let _place = line.lock().await;
        let mut waited = false;
        loop {
            let wait = self.with_state(host, |state| {
                if state.turn.background_owed {
                    return Some(self.refill);
                }
                let wait = take_leaving(
                    &mut state.bucket,
                    Instant::now(),
                    self.burst,
                    self.refill,
                    0,
                );
                if wait.is_none() && waited && state.turn.background_waiting {
                    state.turn.background_owed = true;
                }
                wait
            });
            match wait {
                None => return,
                Some(wait) => {
                    tracing::debug!(
                        host,
                        wait_ms = wait.as_millis(),
                        "budget: pacing a fetch to the host's budget",
                    );
                    waited = true;
                    tokio::time::sleep(wait).await;
                }
            }
        }
    }

    /// A token for `host` for background traffic — subtitle tracks,
    /// the player's and a download's, and a download's fetches through
    /// the proxy. Background requests take their turn one at a time,
    /// in the order they arrived, however many are in flight. With no
    /// one in the host's line, the one whose turn it is takes a token
    /// only while more than the budget's reserve
    /// ([`BACKGROUND_RESERVE`] in the app's budget) is on hand, so the
    /// player's next requests find the reserve there. With the player
    /// waiting in the line it yields until the player has been served
    /// a token it waited for, and then takes the next: while both
    /// wait, they take every other token, and a player filling its
    /// buffer cannot keep it waiting for the whole fill.
    pub(crate) async fn admit_background(&self, host: &str) {
        let lane = self.lane(host);
        let _turn = lane.lock().await;
        let line = self.line(host);
        let _waiting = BackgroundWaiting { budget: self, host };
        self.with_state(host, |state| state.turn.background_waiting = true);
        loop {
            let player_waiting = line.try_lock().is_err();
            let wait = self.with_state(host, |state| {
                let reserve = match (player_waiting, state.turn.background_owed) {
                    (_, true) => 0,
                    (false, false) => self.reserve,
                    (true, false) => return Some(self.refill),
                };
                take_leaving(
                    &mut state.bucket,
                    Instant::now(),
                    self.burst,
                    self.refill,
                    reserve,
                )
            });
            // Looks again within a refill whatever the wait: a turn
            // the player owes it, or a player joining the line, changes
            // what it may take.
            match wait {
                None => return,
                Some(wait) => tokio::time::sleep(wait.min(self.refill)).await,
            }
        }
    }

    /// An unpaced download starts, with up to `in_flight` requests at a
    /// time that pass the budget: every host's tokens are held to what
    /// they leave of the burst until it ends ([`UnpacedRun`]).
    pub(crate) fn unpaced_run(&self, in_flight: u32) -> UnpacedRun<'_> {
        self.unpaced
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .in_flight += in_flight;
        UnpacedRun {
            budget: self,
            in_flight,
        }
    }

    /// The tokens a host is held to beside `in_flight` requests that
    /// pass the budget: what they leave of the burst, never below one,
    /// so the hold alone never empties a bucket.
    fn held_to(&self, in_flight: u32) -> u32 {
        self.burst.saturating_sub(in_flight).max(1)
    }

    /// A bucket for a host first fetched from at `now`: full, or, once
    /// an unpaced download has ended, where the holds left every host,
    /// refilling from then.
    fn new_bucket(&self, now: Instant) -> Bucket {
        self.unpaced
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .untouched
            .unwrap_or_else(|| Bucket::full(self.burst, now))
    }

    /// Spends `n` tokens of `host`'s budget without waiting for them:
    /// requests the app sent past the budget, which the host counts.
    pub(crate) fn spend(&self, host: &str, n: u32) {
        let (burst, refill) = (self.burst, self.refill);
        self.with_state(host, |state| {
            spend_from(&mut state.bucket, Instant::now(), burst, refill, n);
        });
    }

    /// The line of requests waiting at `host`.
    fn line(&self, host: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut lines = self.lines.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(lines.entry(host.to_owned()).or_default())
    }

    /// The lane background requests to `host` take their turn from.
    fn lane(&self, host: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut lanes = self.lanes.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(lanes.entry(host.to_owned()).or_default())
    }

    /// Runs `f` over `host`'s tokens and turn, a full bucket for a host
    /// never fetched from, held to what unpaced downloads there leave
    /// of it.
    fn with_state<T>(&self, host: &str, f: impl FnOnce(&mut HostState) -> T) -> T {
        let mut states = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let state = states.entry(host.to_owned()).or_insert_with(|| HostState {
            bucket: self.new_bucket(Instant::now()),
            turn: Turn::default(),
        });
        self.hold_to_unpaced(&mut state.bucket, Instant::now());
        f(state)
    }

    /// While unpaced downloads run, holds a host's tokens to what their
    /// requests in flight leave of the burst ([`Self::held_to`]).
    fn hold_to_unpaced(&self, bucket: &mut Bucket, now: Instant) {
        let in_flight = self
            .unpaced
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .in_flight;
        if in_flight > 0 {
            hold_to(
                bucket,
                now,
                self.burst,
                self.refill,
                self.held_to(in_flight),
            );
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
