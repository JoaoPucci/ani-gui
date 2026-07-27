//! Slot schedule for background pacing: a ledger of live
//! reservations instead of a single high-water instant, so a
//! reservation whose sleeper is cancelled (a superseded prefetch, an
//! unmounted page) returns its interval to the schedule rather than
//! orphaning 500 ms of dead air — compounding, during an advertised
//! pause, into background warming stalled past recovery.

use tokio::time::{Duration, Instant};

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

    /// Reserve the next paced slot at or after `floor` (an active
    /// pause deadline, or `now` when none). Slots are strictly
    /// increasing across live reservations, so every sleeper holds a
    /// unique instant.
    pub(super) fn reserve(&mut self, interval: Duration, floor: Instant, now: Instant) -> Instant {
        let tail = self
            .reserved
            .last()
            .map_or(self.pace_floor, |newest| *newest + interval);
        let slot = tail.max(self.pace_floor).max(floor).max(now);
        self.reserved.push(slot);
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

#[cfg(test)]
#[path = "reservation_test.rs"]
mod tests;
