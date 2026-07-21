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
/// an outcome; after this window — the meta client's total request
/// timeout — a new trial may start instead of wedging the gate shut.
pub const HALF_OPEN_TRIAL_STALE: Duration = Duration::from_secs(30);

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

struct GateState {
    next_background_at: Instant,
    consecutive_failures: u32,
    open_until: Option<Instant>,
    /// When the current half-open trial probe was admitted; `None`
    /// when no trial is outstanding.
    half_open_trial_at: Option<Instant>,
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
                consecutive_failures: 0,
                open_until: None,
                half_open_trial_at: None,
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
        let wait = {
            let mut s = self.inner.lock().expect("gate lock");
            let now = Instant::now();
            if let Some(until) = s.open_until {
                if now < until {
                    return Err(GateClosed);
                }
                // Cooldown elapsed — half-open: let probes through
                // again. `consecutive_failures` stays where it is, so
                // a single failed probe snaps the breaker shut.
                s.open_until = None;
            }
            let slot = s.next_background_at.max(now);
            s.next_background_at = slot + BACKGROUND_INTERVAL;
            slot - now
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
            // Re-check on wake: a cold-start burst reserves slots
            // before its first requests report failures, so the
            // breaker can open while this caller slept. The slot
            // reservation above stands either way — later probes
            // would be refused anyway while the breaker is open.
            let s = self.inner.lock().expect("gate lock");
            if let Some(until) = s.open_until {
                if Instant::now() < until {
                    return Err(GateClosed);
                }
            }
        }
        Ok(())
    }

    /// Report how the admitted request went. Failures count toward
    /// the breaker; a success resets it.
    pub fn record_outcome(&self, ok: bool) {
        let mut s = self.inner.lock().expect("gate lock");
        if ok {
            s.consecutive_failures = 0;
            s.open_until = None;
            s.half_open_trial_at = None;
        } else {
            s.consecutive_failures += 1;
            if s.consecutive_failures >= FAILURE_THRESHOLD {
                s.open_until = Some(Instant::now() + BREAKER_COOLDOWN);
                s.half_open_trial_at = None;
            }
        }
    }
}

impl Default for ScraperGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "gate_test.rs"]
mod tests;
