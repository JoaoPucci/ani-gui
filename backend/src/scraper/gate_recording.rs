//! Outcome recording for [`ScraperGate`] — a `#[path]` child module
//! of `gate`, split out so each file stays inside the complexity
//! ratchet's per-file bar while keeping the gate's state private to
//! the module tree.

use super::*;

impl ScraperGate {
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
            // A success that STARTED before the pause opened ran
            // against the pre-limit upstream; only fresh evidence
            // clears the window.
            let fresh_for_pause = s.pause_opened_at.is_none_or(|opened| started_at >= opened);
            if fresh_for_pause && s.paused_until.take().is_some() {
                // The schedule holds exactly the reservations real
                // sleepers took (slots floor at the deadline instead
                // of the deadline being pushed into the schedule), so
                // clearing the pause preserves it: new callers queue
                // after the outstanding sleepers and the global
                // pacing survives the early recovery.
                s.pause_opened_at = None;
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
                    s.schedule.clear(now);
                }
                s.half_open_trial_at = None;
            }
        }
    }
}
