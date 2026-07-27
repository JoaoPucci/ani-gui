//! Slot schedule for background pacing: a ledger of live
//! reservations instead of a single high-water instant, so a
//! reservation whose sleeper is cancelled (a superseded prefetch, an
//! unmounted page) returns its interval to the schedule rather than
//! orphaning 500 ms of dead air — compounding, during an advertised
//! pause, into background warming stalled past recovery.

use std::sync::Mutex;
use tokio::time::{Duration, Instant};

use super::gate::GateState;

/// The gate's background slot ledger. Consumed slots advance a pacing
/// floor (their request actually fired); cancelled reservations just
/// leave the ledger, so only real traffic shapes future pacing.
pub(super) struct SlotSchedule {
    /// Earliest slot the next reservation may take, based on
    /// requests that actually fired: consuming a slot advances it
    /// one interval past that slot. Cancelled reservations never
    /// advance it.
    pace_floor: Instant,
    /// Live reservations in slot order: background callers sleeping
    /// toward their slot. Each new reservation queues one interval
    /// after the newest live slot, so clearing a pause early still
    /// leaves real sleepers ahead of new callers.
    reserved: Vec<Instant>,
}

impl SlotSchedule {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            pace_floor: now,
            reserved: Vec::new(),
        }
    }

    /// Reserve the earliest paced slot at or after `floor` (an active
    /// pause deadline, or `now` when none) that keeps a full interval
    /// clear of every live reservation. Intervals vacated by released
    /// sleepers are refilled — cancelling early or middle sleepers
    /// must not strand new work behind a survivor's conservative slot
    /// as if nothing had been released. Every sleeper holds a unique
    /// instant: a candidate colliding with a live slot jumps past it.
    pub(super) fn reserve(&mut self, interval: Duration, floor: Instant, now: Instant) -> Instant {
        let mut slot = self.pace_floor.max(floor).max(now);
        for taken in &self.reserved {
            if *taken + interval <= slot {
                continue;
            }
            if slot + interval <= *taken {
                break;
            }
            slot = *taken + interval;
        }
        let at = self.reserved.partition_point(|taken| *taken < slot);
        self.reserved.insert(at, slot);
        slot
    }

    /// Return an unfired slot to the schedule: the sleeper holding it
    /// was cancelled or refused, so no request will ever occupy it.
    /// Unknown slots are a no-op — the ledger may have been cleared
    /// wholesale while the sleeper slept.
    pub(super) fn release(&mut self, slot: Instant) {
        if let Some(i) = self.reserved.iter().position(|r| *r == slot) {
            self.reserved.remove(i);
        }
    }

    /// Consume a slot whose request is about to fire: later
    /// reservations stay at least one interval behind it even after
    /// the ledger empties.
    pub(super) fn consume(&mut self, interval: Duration, slot: Instant) {
        self.release(slot);
        self.pace_floor = self.pace_floor.max(slot + interval);
    }

    /// Drop every queued reservation and restart pacing at `now`.
    /// Used at the breaker's closed-to-open transition: every queued
    /// sleeper will be refused at wake, and the half-open trial must
    /// not wait behind their doomed slots.
    pub(super) fn clear(&mut self, now: Instant) {
        self.reserved.clear();
        self.pace_floor = now;
    }
}

/// Holds a background caller's slot reservation across its sleeps.
/// The slot is taken out (`slot.take()`) when consumed on the fire
/// path or traded for a fresh one during a pause re-sleep; if the
/// admit future is dropped mid-sleep or refused at wake with the
/// reservation still held, `Drop` returns the slot to the schedule.
/// The drop-path lock is deadlock-free: an early return unwinds the
/// wake loop's `MutexGuard` before the guard itself drops.
pub(super) struct SlotGuard<'a> {
    pub(super) gate: &'a Mutex<GateState>,
    pub(super) slot: Option<Instant>,
}

impl Drop for SlotGuard<'_> {
    fn drop(&mut self) {
        if let Some(slot) = self.slot.take() {
            if let Ok(mut s) = self.gate.lock() {
                s.schedule.release(slot);
            }
        }
    }
}

#[cfg(test)]
#[path = "reservation_test.rs"]
mod tests;
