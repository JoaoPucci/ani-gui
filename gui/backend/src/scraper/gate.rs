//! Global admission gate for provider traffic.
//!
//! Cold caches make the home page warm every rail entry at once, and
//! each probe fans out over the primary title plus every alt title.
//! Ungated, one launch fired hundreds of searches in under a minute,
//! the provider rate-limited the IP, and the user's very first play click
//! died in resolution with "No results found!". The gate exists so
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
//!   the provider throttles with 200-status HTML pages) and stays open
//!   for [`BREAKER_COOLDOWN`]. While open, background callers skip
//!   the network instantly instead of deepening the limit; their
//!   cache rows simply stay unwritten, which every consumer already
//!   renders as "unknown, don't gate". After the cooldown one probe
//!   is let through; a single failure re-opens, a success resets.

use std::sync::Mutex;
use tokio::time::{Duration, Instant};

use super::reservation::{SlotGuard, SlotSchedule};

pub use super::outcome::ScrapeOutcome;

/// Minimum spacing between background scraper requests. Matches the
/// cadence the warm loop always intended (one probe per 500 ms) but
/// enforced per *request*, so a probe's alt-title fan-out can no
/// longer burst.
pub const BACKGROUND_INTERVAL: Duration = Duration::from_millis(500);

/// Consecutive failures that open the breaker. Three is enough to
/// distinguish "the provider is refusing us" from a flaky single request
/// without burning hundreds of doomed calls discovering it.
pub const FAILURE_THRESHOLD: u32 = 3;

/// How long the breaker stays open before letting one background
/// probe through again.
pub const BREAKER_COOLDOWN: Duration = Duration::from_secs(60);

/// Sanity ceiling on the advertised retry hint. The hint is
/// untrusted upstream input: an enormous value must neither overflow
/// `Instant` arithmetic (a panic under the gate lock poisons the
/// mutex for the process lifetime) nor stall background work for
/// hours on the upstream's say-so.
pub const MAX_ADVERTISED_PAUSE: Duration = Duration::from_secs(600);

/// How long an unreported half-open trial blocks the next one. A
/// trial whose future was dropped (cancelled prefetch) never records
/// an outcome; after this window a new trial may start instead of
/// wedging the gate shut. Sized past the longest gated operation —
/// the 60 s prefetch resolve timeout, not just the 30 s meta
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

pub(super) struct GateState {
    /// Background slot ledger: live reservations plus the pacing
    /// floor of actually-fired requests. A cancelled sleeper's slot
    /// returns to the schedule instead of orphaning dead air.
    pub(super) schedule: SlotSchedule,
    /// Advertised-window pause: background admits WAIT until this
    /// instant (then resume paced) instead of being refused. Opened
    /// by a single typed rate-limit outcome; cleared by a fresh
    /// success. Distinct from the breaker: the upstream told us when
    /// to come back, so refusing work and losing rows would be pure
    /// waste.
    paused_until: Option<Instant>,
    /// When the current pause first opened. Successes that STARTED
    /// before this instant ran against the pre-limit upstream — stale
    /// evidence that must not clear the newer window, the mirror of
    /// the breaker's `opened_at` filter. Kept at the earliest opening
    /// across overlapping rate limits.
    pause_opened_at: Option<Instant>,
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
/// provider request goes through [`ScraperGate::admit`] first and
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
                schedule: SlotSchedule::new(Instant::now()),
                paused_until: None,
                pause_opened_at: None,
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
        self.admit_chained(prio, None).await?;
        Ok(())
    }

    /// [`ScraperGate::admit`] for callers whose logical request is a
    /// CHAIN of provider fetches (a native resolve: search, probes,
    /// the episode chain). Pacing is identical — every fetch takes
    /// its own slot — but the half-open trial sanctions the chain,
    /// not a single fetch: the admit that takes the trial hands the
    /// sanction back, and the holder presents it on its further
    /// admits, which pass while the gate still holds that exact
    /// stamp. Everyone else stays refused until the chain's outcome
    /// is recorded or the trial goes stale. Without this, a chain's
    /// second fetch is refused by its own trial, the refusal is
    /// recorded as failure, and the breaker re-opens on itself.
    ///
    /// # Errors
    /// [`GateClosed`] for background admits while the breaker is
    /// open, unless `held` is the outstanding trial's sanction.
    pub async fn admit_chained(
        &self,
        prio: ScrapePriority,
        held: Option<Instant>,
    ) -> Result<Option<Instant>, GateClosed> {
        if prio == ScrapePriority::Interactive {
            return Ok(None);
        }
        let (slot, mut trial_stamp) = {
            let mut s = self.inner.lock().expect("gate lock");
            let now = Instant::now();
            // Advertised-window pause: schedule this caller a paced
            // slot at/after the window's end and sleep until then —
            // pause-and-resume, not refusal. The slot floors at any
            // active pause deadline: the window is enforced in slot
            // selection itself, so no schedule bookkeeping elsewhere
            // (the breaker's closed-to-open reset included) can punch
            // through it. A fresh success clears `paused_until` but
            // queued sleepers keep their conservative slots.
            if s.paused_until.is_some_and(|paused| now >= paused) {
                s.paused_until = None;
                s.pause_opened_at = None;
            }
            let trial_stamp = breaker_gate(&mut s, now, held)?;
            let floor = s.paused_until.unwrap_or(now);
            (
                s.schedule.reserve(BACKGROUND_INTERVAL, floor, now),
                trial_stamp,
            )
        };
        // The reservation lives in a guard from here on: dropping the
        // admit future mid-sleep (a superseded prefetch, an unmounted
        // page) or being refused at wake releases the slot back to
        // the schedule instead of orphaning 500 ms of dead air per
        // cancellation — which, during a pause, compounded into
        // background warming stalled past recovery.
        let mut guard = SlotGuard {
            gate: &self.inner,
            slot: Some(slot),
        };
        if Instant::now() < slot {
            tokio::time::sleep_until(slot).await;
            // Re-check on wake: a cold-start burst reserves slots
            // before its first requests report anything, so the
            // breaker can open — or an advertised pause can start or
            // extend — while this caller slept. Breaker: refuse —
            // the half-open trial is exempt only while the gate still
            // holds the exact sanction stamp that admitted it. Any
            // clearing (a fresh success, a rate limit, a new breaker
            // cycle) retires the sanction, and the sleeper submits to
            // the current breaker like everyone else instead of
            // piercing a cooldown that never authorized it. Pause:
            // EVERY caller, the trial included, trades its slot for a
            // fresh one past the newest deadline and sleeps again —
            // bounded, since each iteration only re-sleeps for a
            // window recorded after the previous wake.
            loop {
                let resleep = {
                    let mut s = self.inner.lock().expect("gate lock");
                    let now = Instant::now();
                    // breaker_gate's held check IS the sanction test:
                    // a stamp the gate still holds passes straight
                    // through; anything else re-submits to the
                    // current breaker like everyone else.
                    trial_stamp = breaker_gate(&mut s, now, trial_stamp)?;
                    s.paused_until.filter(|paused| now < *paused).map(|paused| {
                        if let Some(old) = guard.slot.take() {
                            s.schedule.release(old);
                        }
                        let next = s.schedule.reserve(BACKGROUND_INTERVAL, paused, now);
                        guard.slot = Some(next);
                        next
                    })
                };
                match resleep {
                    None => break,
                    Some(next) => tokio::time::sleep_until(next).await,
                }
            }
        }
        let mut s = self.inner.lock().expect("gate lock");
        let fired = guard.slot.take().expect("fire path holds the reservation");
        s.schedule.consume(BACKGROUND_INTERVAL, fired);
        Ok(trial_stamp)
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
                if s.last_recovery_at
                    .is_some_and(|recovered| started_at < recovered)
                {
                    // The request observed the pre-recovery upstream;
                    // a newer accepted success already proved that
                    // state is gone — same filter as the untyped
                    // failure path.
                    return;
                }
                let now = Instant::now();
                // The hint is untrusted input: clamp before the
                // Instant addition so a hostile value can neither
                // overflow (poisoning the mutex via panic) nor stall
                // background work for hours.
                let window = retry_after
                    .unwrap_or(BREAKER_COOLDOWN)
                    .min(MAX_ADVERTISED_PAUSE);
                let until = now + window;
                // Keep the later deadline if two rate limits overlap,
                // and the earliest opening as the staleness boundary.
                s.paused_until = Some(s.paused_until.map_or(until, |prev| prev.max(until)));
                // Newest opening owns staleness (a success must
                // postdate every observed limit); the deadline keeps
                // the maximum above.
                s.pause_opened_at = Some(now);
                // If this was the half-open trial reporting, that IS
                // its outcome: the pause owns recovery timing now, so
                // the breaker's cooldown/trial state stands down
                // rather than refusing everyone for the trial-stale
                // window after the pause ends.
                s.half_open_trial_at = None;
                s.open_until = None;
                s.opened_at = None;
            }
        }
    }
}

// `record_outcome` lives in a `#[path]` child module so its
// complexity counts against its own file while the gate's state
// stays private to this module tree.
#[path = "gate_recording.rs"]
mod recording;

/// Breaker check under the gate lock: refuses while the breaker is
/// open, and once the cooldown elapses hands the half-open trial role
/// to exactly one caller — everyone else stays refused until the
/// trial reports or goes stale. The returned stamp is the sanction
/// itself: the caller holds it as proof, and the exemption lasts only
/// while `half_open_trial_at` still equals it — any state clearing
/// retires the sanction along with the cycle that granted it. A trial
/// whose future was dropped stops blocking after
/// [`HALF_OPEN_TRIAL_STALE`]. `consecutive_failures` is left as-is,
/// so a single failed trial snaps the breaker shut.
fn breaker_gate(
    s: &mut GateState,
    now: Instant,
    held: Option<Instant>,
) -> Result<Option<Instant>, GateClosed> {
    let Some(until) = s.open_until else {
        return Ok(None);
    };
    // The holder of the outstanding trial's sanction IS the trial:
    // its chain's further admits pass on the same stamp, for as long
    // as the gate still holds exactly that stamp.
    if held.is_some() && held == s.half_open_trial_at {
        return Ok(held);
    }
    if now < until {
        return Err(GateClosed);
    }
    if let Some(t) = s.half_open_trial_at {
        if now - t < HALF_OPEN_TRIAL_STALE {
            return Err(GateClosed);
        }
    }
    s.half_open_trial_at = Some(now);
    Ok(Some(now))
}

impl Default for ScraperGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "gate_test.rs"]
mod tests;
