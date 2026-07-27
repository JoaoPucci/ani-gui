//! Global admission gate for allanime scraper traffic.
//!
//! Cold caches make the home page warm every rail entry at once, and
//! each probe fans out over the primary title plus every alt title.
//! Ungated, one launch fired hundreds of searches in under a minute,
//! allanime rate-limited the IP, and the user's very first play click
//! died inside ani-cli with "No results found!". The gate exists so
//! background traffic can never poison the connection the user's next
//! click depends on:
//!
//! - **Background** admits (rail warms, prefetch, home-loader probes)
//!   are spaced at [`BACKGROUND_INTERVAL`] per request and refused
//!   outright while the breaker is open.
//! - **Interactive** admits (a user clicking play, a detail-page CTA
//!   probe) always pass — one user-initiated request is never the
//!   problem, and blocking it would trade a working click for cache
//!   hygiene.
//! - The breaker opens after [`FAILURE_THRESHOLD`] *consecutive*
//!   scraper failures (transport errors, 429/5xx, or garbage bodies —
//!   allanime throttles with 200-status HTML pages) and stays open
//!   for [`BREAKER_COOLDOWN`]. While open, background callers skip
//!   the network instantly instead of deepening the limit; their
//!   cache rows simply stay unwritten, which every consumer already
//!   renders as "unknown, don't gate". After the cooldown one probe
//!   is let through; a single failure re-opens, a success resets.

use std::sync::Mutex;
use tokio::time::{Duration, Instant};

/// Minimum spacing between background scraper requests. Matches the
/// cadence the warm loop always intended (one probe per 500 ms) but
/// enforced per *request*, so a probe's alt-title fan-out can no
/// longer burst.
pub const BACKGROUND_INTERVAL: Duration = Duration::from_millis(500);

/// Consecutive failures that open the breaker. Three is enough to
/// distinguish "allanime is refusing us" from a flaky single request
/// without burning hundreds of doomed calls discovering it.
pub const FAILURE_THRESHOLD: u32 = 3;

/// How long the breaker stays open before letting one background
/// probe through again.
pub const BREAKER_COOLDOWN: Duration = Duration::from_secs(60);

/// How long an unreported half-open trial blocks the next one. A
/// trial whose future was dropped (cancelled prefetch) never records
/// an outcome; after this window a new trial may start instead of
/// wedging the gate shut. Sized past the longest gated operation —
/// the 60 s prefetch ani-cli spawn timeout, not just the 30 s meta
/// client — with margin, or a slow spawn-trial still legitimately
/// running would see a second trial sanctioned beside it.
pub const HALF_OPEN_TRIAL_STALE: Duration = Duration::from_secs(90);

/// Who is asking for a scraper slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrapePriority {
    /// A user is waiting on the result (play click, detail-page CTA).
    Interactive,
    /// Opportunistic cache filling (warm, prefetch, rail probes).
    Background,
}

/// Returned to background callers while the breaker is open. Callers
/// treat it like a transient network failure: skip the request, leave
/// the cache row unwritten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateClosed;

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

struct GateState {
    next_background_at: Instant,
    /// Advertised-window pause: background admits WAIT until this
    /// instant (then resume paced) instead of being refused. Opened
    /// by a single typed rate-limit outcome; cleared by a fresh
    /// success. Distinct from the breaker: the upstream told us when
    /// to come back, so refusing work and losing rows would be pure
    /// waste.
    paused_until: Option<Instant>,
    consecutive_failures: u32,
    open_until: Option<Instant>,
    /// When the breaker last opened; `None` when it has never opened
    /// or a fresh success closed it. Successes started before this
    /// instant are stale evidence and cannot close the breaker.
    opened_at: Option<Instant>,
    /// When the current half-open trial probe was admitted; `None`
    /// when no trial is outstanding.
    half_open_trial_at: Option<Instant>,
    /// When the last accepted success was recorded. Failures whose
    /// `started_at` predates this ran against the pre-recovery
    /// upstream — stale evidence that must not count toward the
    /// breaker, the mirror of the stale-success filter against
    /// `opened_at`.
    last_recovery_at: Option<Instant>,
}

/// See the module docs. One instance lives in `AppState`; every
/// allanime request goes through [`ScraperGate::admit`] first and
/// reports back via [`ScraperGate::record_outcome`].
pub struct ScraperGate {
    inner: Mutex<GateState>,
}

impl ScraperGate {
    /// A fresh gate: breaker closed, first background slot available
    /// immediately.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(GateState {
                next_background_at: Instant::now(),
                paused_until: None,
                consecutive_failures: 0,
                open_until: None,
                opened_at: None,
                half_open_trial_at: None,
                last_recovery_at: None,
            }),
        }
    }

    /// Wait for (background) or immediately take (interactive) a
    /// scraper slot. The wait happens outside the lock: concurrent
    /// background callers each reserve the next 500 ms slot under the
    /// lock and then sleep until their slot, so a fan-out queues up
    /// evenly instead of bursting.
    ///
    /// # Errors
    /// [`GateClosed`] for background admits while the breaker is open.
    pub async fn admit(&self, prio: ScrapePriority) -> Result<(), GateClosed> {
        if prio == ScrapePriority::Interactive {
            return Ok(());
        }
        let (wait, is_trial) = {
            let mut s = self.inner.lock().expect("gate lock");
            let now = Instant::now();
            // Advertised-window pause: schedule this caller a paced
            // slot at/after the window's end and sleep until then —
            // pause-and-resume, not refusal. `next_background_at`
            // was pushed past the window when the pause opened, so
            // the ordinary slot math below does the queuing; a fresh
            // success clears `paused_until` but queued sleepers keep
            // their conservative slots.
            if let Some(paused) = s.paused_until {
                if now >= paused {
                    s.paused_until = None;
                }
            }
            let is_trial = breaker_gate(&mut s, now)?;
            let slot = s.next_background_at.max(now);
            s.next_background_at = slot + BACKGROUND_INTERVAL;
            (slot - now, is_trial)
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
            // Re-check on wake: a cold-start burst reserves slots
            // before its first requests report failures, so the
            // breaker can open while this caller slept. The slot
            // reservation above stands either way — later probes
            // would be refused anyway while the breaker is open. The
            // half-open trial itself skips this: it was sanctioned
            // under the same open state it would now observe.
            if !is_trial {
                let mut s = self.inner.lock().expect("gate lock");
                breaker_gate(&mut s, Instant::now())?;
            }
        }
        Ok(())
    }

    /// Typed outcome reporting: like [`ScraperGate::record_outcome`],
    /// but a [`ScrapeOutcome::RateLimited`] opens an advertised-window
    /// pause immediately — background admits then WAIT through the
    /// window and resume paced, instead of being refused and losing
    /// their rows. Without a hint the window falls back to
    /// [`BREAKER_COOLDOWN`]. Interactive admits ignore the window as
    /// they ignore the breaker.
    pub fn record(&self, outcome: ScrapeOutcome, started_at: Instant) {
        match outcome {
            ScrapeOutcome::Success => self.record_outcome(true, started_at),
            ScrapeOutcome::Failure => self.record_outcome(false, started_at),
            ScrapeOutcome::RateLimited { retry_after } => {
                let mut s = self.inner.lock().expect("gate lock");
                let now = Instant::now();
                let window = retry_after.unwrap_or(BREAKER_COOLDOWN);
                let until = now + window;
                // Keep the later deadline if two rate limits overlap.
                s.paused_until = Some(match s.paused_until {
                    Some(prev) => prev.max(until),
                    None => until,
                });
                // Resume paced from the window's end, not from a
                // schedule queued before the pause.
                s.next_background_at = s.next_background_at.max(until);
            }
        }
    }

    /// Report how the admitted request went. `started_at` is when
    /// the caller began the request (capture `Instant::now()` before
    /// firing): failures count toward the breaker regardless, but a
    /// success only closes an open breaker when the request started
    /// after the breaker opened — a slow pre-storm request reporting
    /// success later is stale evidence, not proof of recovery.
    pub fn record_outcome(&self, ok: bool, started_at: Instant) {
        let mut s = self.inner.lock().expect("gate lock");
        if ok {
            if let Some(opened) = s.opened_at {
                if started_at < opened {
                    // Pre-storm evidence: the request was already in
                    // flight when the breaker opened, so its success
                    // says nothing about the upstream's state NOW.
                    // Leave the breaker (and the failure run) alone.
                    return;
                }
            }
            s.consecutive_failures = 0;
            s.open_until = None;
            s.opened_at = None;
            s.half_open_trial_at = None;
            if s.paused_until.take().is_some() {
                // The pause pushed the resume schedule out to the
                // window's end; a proven recovery pulls it back so
                // new admits aren't still waiting on a window that
                // no longer applies. Sleepers already queued keep
                // their conservative slots.
                s.next_background_at = Instant::now();
            }
            // The boundary is when the successful request STARTED,
            // advanced monotonically — a slow success that lands late
            // proves recovery only as of its own start; requests that
            // began after it may be watching a newer rate-limit
            // episode and their failures stay fresh evidence.
            s.last_recovery_at = Some(match s.last_recovery_at {
                Some(prev) => prev.max(started_at),
                None => started_at,
            });
        } else {
            if let Some(recovered) = s.last_recovery_at {
                if started_at < recovered {
                    // The request ran against the pre-recovery
                    // upstream; a newer accepted success already
                    // proved the state it observed is gone.
                    return;
                }
            }
            s.consecutive_failures += 1;
            if s.consecutive_failures >= FAILURE_THRESHOLD {
                let now = Instant::now();
                s.open_until = Some(now + BREAKER_COOLDOWN);
                if s.opened_at.is_none() {
                    // Closed → open transition only: stragglers that
                    // fail while already open must not move the
                    // staleness boundary (a fresh interactive success
                    // started during the cooldown could never close
                    // the breaker), and the queued slot schedule is
                    // dropped so the half-open trial doesn't wait
                    // behind reservations from callers that will all
                    // be refused at wake.
                    s.opened_at = Some(now);
                    s.next_background_at = now;
                }
                s.half_open_trial_at = None;
            }
        }
    }
}

/// Classify a request result for [`ScraperGate::record`]: the typed
/// in-band rate limit carries its advertised hint to the gate, every
/// other error folds to an untyped failure.
pub fn outcome_of<T>(r: &Result<T, crate::error::AniError>) -> ScrapeOutcome {
    match r {
        Ok(_) => ScrapeOutcome::Success,
        Err(crate::error::AniError::RateLimited { retry_after_secs }) => {
            ScrapeOutcome::RateLimited {
                retry_after: retry_after_secs.map(Duration::from_secs),
            }
        }
        Err(_) => ScrapeOutcome::Failure,
    }
}

/// Breaker check under the gate lock: refuses while the breaker is
/// open, and once the cooldown elapses hands the half-open trial role
/// to exactly one caller (returning `true` for it) — everyone else
/// stays refused until the trial reports or goes stale. A trial whose
/// future was dropped stops blocking after [`HALF_OPEN_TRIAL_STALE`].
/// `consecutive_failures` is left as-is, so a single failed trial
/// snaps the breaker shut.
fn breaker_gate(s: &mut GateState, now: Instant) -> Result<bool, GateClosed> {
    let Some(until) = s.open_until else {
        return Ok(false);
    };
    if now < until {
        return Err(GateClosed);
    }
    if let Some(t) = s.half_open_trial_at {
        if now - t < HALF_OPEN_TRIAL_STALE {
            return Err(GateClosed);
        }
    }
    s.half_open_trial_at = Some(now);
    Ok(true)
}

impl Default for ScraperGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "gate_test.rs"]
mod tests;
