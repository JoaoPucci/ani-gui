//! Tests for `super::SlotSchedule`. Extracted via `#[path]` so the
//! inline `mod tests { ... }` block doesn't count toward the file's
//! CCN — per `project_crap_inline_test_gotcha`.

use super::*;

const INTERVAL: Duration = Duration::from_millis(500);

fn base() -> Instant {
    Instant::now()
}

#[test]
fn reservations_pace_one_interval_apart() {
    let now = base();
    let mut s = SlotSchedule::new(now);
    assert_eq!(s.reserve(INTERVAL, now, now), now);
    assert_eq!(s.reserve(INTERVAL, now, now), now + INTERVAL);
    assert_eq!(s.reserve(INTERVAL, now, now), now + 2 * INTERVAL);
}

#[test]
fn the_floor_defers_the_slot_and_the_queue_forms_behind_it() {
    let now = base();
    let deadline = now + Duration::from_secs(120);
    let mut s = SlotSchedule::new(now);
    assert_eq!(s.reserve(INTERVAL, deadline, now), deadline);
    assert_eq!(s.reserve(INTERVAL, deadline, now), deadline + INTERVAL);
}

#[test]
fn releasing_a_reservation_returns_its_slot() {
    let now = base();
    let deadline = now + Duration::from_secs(120);
    let mut s = SlotSchedule::new(now);
    let slot = s.reserve(INTERVAL, deadline, now);
    s.release(slot);
    // The dead sleeper no longer shapes the schedule: with the pause
    // gone, the next caller admits at `now`, not behind the deadline.
    assert_eq!(s.reserve(INTERVAL, now, now), now);
}

#[test]
fn releasing_an_unknown_slot_is_a_no_op() {
    let now = base();
    let mut s = SlotSchedule::new(now);
    let slot = s.reserve(INTERVAL, now, now);
    s.release(slot + Duration::from_secs(9));
    assert_eq!(s.reserve(INTERVAL, now, now), slot + INTERVAL);
}

#[test]
fn a_mid_queue_release_refills_the_hole_while_keeping_pacing() {
    let now = base();
    let mut s = SlotSchedule::new(now);
    let first = s.reserve(INTERVAL, now, now);
    let second = s.reserve(INTERVAL, now, now);
    s.release(first);
    // The survivor keeps its absolute slot, and the vacated interval
    // goes to the next caller — a full interval before the survivor,
    // never on top of it.
    assert_eq!(s.reserve(INTERVAL, now, now), first);
    assert_eq!(s.reserve(INTERVAL, now, now), second + INTERVAL);
}

#[test]
fn consuming_a_slot_keeps_future_pacing_behind_it() {
    let now = base();
    let mut s = SlotSchedule::new(now);
    let slot = s.reserve(INTERVAL, now, now);
    s.consume(INTERVAL, slot);
    // The request fired: even with the ledger empty, the next
    // reservation stays a full interval behind the fired slot.
    assert_eq!(s.reserve(INTERVAL, now, now), slot + INTERVAL);
}

#[test]
fn clear_drops_the_queue_and_restarts_pacing() {
    let now = base();
    let deadline = now + Duration::from_secs(120);
    let mut s = SlotSchedule::new(now);
    s.reserve(INTERVAL, deadline, now);
    s.reserve(INTERVAL, deadline, now);
    let later = now + Duration::from_secs(7);
    s.clear(later);
    assert_eq!(s.reserve(INTERVAL, later, later), later);
}
