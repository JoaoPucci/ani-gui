//! One budget for every request this app sends AniList.
//!
//! AniList limits the API per client — at present 30 requests a
//! minute while it runs degraded, 90 otherwise — and answers a client
//! past it with a 429 and a one-minute timeout, during which further
//! requests are refused too. Nothing in
//! the app owns that budget on its own: trending, banners, episode
//! thumbnails, airing, the id bridges and the signed-in account's
//! reads and writes all spend it, and a cold home page asks for
//! several of them per card at once. So every GraphQL transport — the
//! public POST, the `streamingEpisodes` lookup and the authenticated
//! account POST — sends through the gate kept here for AniList's API
//! host, and the gate does two things:
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
//! The gate is chosen by the request's host. AniList's API host has
//! one for the life of the process. Any other host — in practice only
//! a test's mock server — gets a fresh gate per call unless a test
//! shares one
//! ([`share_for_tests`]), so tests that never asked for pacing do not
//! pace each other through a reused mock port.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, LazyLock, Mutex};
use tokio::time::Instant;

use crate::error::{AniError, Result};

#[path = "anilist_gate_answer.rs"]
mod answer;
#[path = "anilist_gate_pace.rs"]
mod pace;
use answer::{authority, not_sent, stay_away};
pub(crate) use pace::{Class, Ledger, Pace, ANILIST};

tokio::task_local! {
    static CLASS: Class;
}

/// Run `f` with every AniList request it sends counted as
/// [`Class::Background`].
pub(crate) async fn in_background<F: Future>(f: F) -> F::Output {
    CLASS.scope(Class::Background, f).await
}

/// `f`, with every AniList request it sends counted in the class of
/// whoever calls this — for work handed to a task of its own, which
/// would otherwise count as interactive whoever asked for it.
pub(crate) fn in_callers_class<F: Future>(f: F) -> impl Future<Output = F::Output> {
    CLASS.scope(current_class(), f)
}

fn current_class() -> Class {
    CLASS.try_with(|c| *c).unwrap_or(Class::Interactive)
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
            let (now, at) = {
                let mut ledger = self.ledger();
                // Read under the lock: a send settled by another task
                // between a clock read and the lock would land after
                // `now`, and this send would go in behind it.
                let now = Instant::now();
                let at = ledger.ready_at(&self.pace, class, now);
                if at <= now {
                    ledger.record_send(&self.pace, now);
                    return Ok(now);
                }
                (now, at)
            };
            if at > deadline {
                return Err(not_sent(at - now, class));
            }
            tokio::time::sleep_until(at).await;
        }
    }
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
            ?wait,
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
#[path = "anilist_gate_test.rs"]
mod tests;
