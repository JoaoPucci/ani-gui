//! Tests for `crate::commands::availability_ttl`.

use super::*;

const NOW: u64 = 1_791_000_000;
const DAY: u64 = 24 * 60 * 60;
const HOUR: u64 = 60 * 60;

#[test]
fn no_schedule_keeps_the_window() {
    assert_eq!(bounded_by_next_airing(DAY, None, NOW), DAY);
}

#[test]
fn a_drop_inside_the_window_cuts_it_at_the_drop_plus_grace() {
    assert_eq!(
        bounded_by_next_airing(DAY, Some(NOW + 2 * HOUR), NOW),
        2 * HOUR + GRACE_SECS
    );
}

#[test]
fn a_drop_past_the_window_leaves_it_alone() {
    assert_eq!(bounded_by_next_airing(DAY, Some(NOW + 5 * DAY), NOW), DAY);
}

#[test]
fn a_passed_airing_re_asks_within_the_floor() {
    // The schedule row is stale or the episode just dropped.
    assert_eq!(
        bounded_by_next_airing(DAY, Some(NOW - HOUR), NOW),
        FLOOR_SECS
    );
    assert_eq!(bounded_by_next_airing(DAY, Some(NOW), NOW), FLOOR_SECS);
}

#[test]
fn an_imminent_drop_never_cuts_below_the_floor() {
    assert_eq!(
        bounded_by_next_airing(DAY, Some(NOW + 60), NOW),
        GRACE_SECS + 60
    );
    assert!(bounded_by_next_airing(DAY, Some(NOW + 1), NOW) >= FLOOR_SECS);
}

proptest::proptest! {
    #[test]
    fn the_cut_only_ever_shortens(
        base in 0u64..(60 * DAY),
        at in proptest::option::of(0u64..(2 * NOW)),
    ) {
        let ttl = bounded_by_next_airing(base, at, NOW);
        proptest::prop_assert!(ttl <= base);
        proptest::prop_assert!(ttl >= base.min(FLOOR_SECS));
    }

    #[test]
    fn a_later_drop_never_gives_a_shorter_window(
        base in 0u64..(60 * DAY),
        a in 0u64..(2 * NOW),
        b in 0u64..(2 * NOW),
    ) {
        let (early, late) = if a <= b { (a, b) } else { (b, a) };
        proptest::prop_assert!(
            bounded_by_next_airing(base, Some(early), NOW)
                <= bounded_by_next_airing(base, Some(late), NOW)
        );
    }
}

// --- a schedule that arrives after the row -------------------------------

#[test]
fn a_row_written_before_its_schedule_is_cut_at_the_drop() {
    // Written an hour ago with the ongoing day; the drop is in two hours.
    assert_eq!(
        rescheduled_ttl(DAY, HOUR, DAY, Some(NOW + 2 * HOUR), NOW),
        Some(HOUR + 2 * HOUR + GRACE_SECS)
    );
}

#[test]
fn a_row_already_inside_the_cut_is_left_alone() {
    // Written knowing the schedule: two hours left, drop in five.
    assert_eq!(
        rescheduled_ttl(3 * HOUR, HOUR, DAY, Some(NOW + 5 * HOUR), NOW),
        None
    );
}

#[test]
fn a_passed_airing_leaves_the_row_the_floor() {
    assert_eq!(
        rescheduled_ttl(DAY, HOUR, DAY, Some(NOW - HOUR), NOW),
        Some(HOUR + FLOOR_SECS)
    );
}

#[test]
fn no_schedule_leaves_the_row_alone() {
    assert_eq!(rescheduled_ttl(DAY, HOUR, DAY, None, NOW), None);
}

#[test]
fn a_finished_show_s_month_is_left_alone() {
    assert_eq!(
        rescheduled_ttl(30 * DAY, HOUR, DAY, Some(NOW + 2 * HOUR), NOW),
        None
    );
}

#[test]
fn an_expired_row_is_left_alone() {
    assert_eq!(
        rescheduled_ttl(DAY, 2 * DAY, DAY, Some(NOW + 2 * HOUR), NOW),
        None
    );
    assert_eq!(
        rescheduled_ttl(DAY, DAY, DAY, Some(NOW + 2 * HOUR), NOW),
        None
    );
}

proptest::proptest! {
    #[test]
    fn a_reschedule_only_ever_shortens_a_live_row(
        ttl in 0u64..(60 * DAY),
        age in 0u64..(60 * DAY),
        at in proptest::option::of(0u64..(2 * NOW)),
    ) {
        if let Some(cut) = rescheduled_ttl(ttl, age, DAY, at, NOW) {
            proptest::prop_assert!(cut < ttl);
            // Still live when cut: never expired by the reschedule itself.
            proptest::prop_assert!(cut > age);
            proptest::prop_assert!(ttl <= DAY);
        }
    }

    #[test]
    fn a_reschedule_matches_a_write_made_now(
        ttl in 1u64..=DAY,
        age in 0u64..DAY,
        at in 0u64..(2 * NOW),
    ) {
        proptest::prop_assume!(age < ttl);
        let remaining = ttl - age;
        let fresh = bounded_by_next_airing(remaining, Some(at), NOW);
        let expected = (fresh < remaining).then_some(age + fresh);
        proptest::prop_assert_eq!(rescheduled_ttl(ttl, age, DAY, Some(at), NOW), expected);
    }
}

// --- negative rows ------------------------------------------------------

#[test]
fn a_negative_row_written_before_its_schedule_is_cut_past_the_drop() {
    assert_eq!(
        rescheduled_negative_ttl(DAY, HOUR, DAY, Some(NOW + 2 * HOUR), NOW),
        Some(HOUR + 2 * HOUR + NEGATIVE_GRACE_SECS)
    );
}

#[test]
fn a_negative_row_with_a_passed_airing_keeps_the_floor() {
    assert_eq!(
        rescheduled_negative_ttl(DAY, HOUR, DAY, Some(NOW - HOUR), NOW),
        Some(HOUR + FLOOR_SECS)
    );
}

#[test]
fn negative_rows_the_cut_does_not_concern_are_left_alone() {
    let at = Some(NOW + 2 * HOUR);
    // A finished show's week.
    assert_eq!(rescheduled_negative_ttl(7 * DAY, HOUR, DAY, at, NOW), None);
    // Expired.
    assert_eq!(rescheduled_negative_ttl(DAY, DAY, DAY, at, NOW), None);
    // Already inside the cut.
    assert_eq!(
        rescheduled_negative_ttl(4 * HOUR, HOUR, DAY, Some(NOW + 5 * HOUR), NOW),
        None
    );
    // No schedule.
    assert_eq!(rescheduled_negative_ttl(DAY, HOUR, DAY, None, NOW), None);
}

proptest::proptest! {
    #[test]
    fn a_negative_reschedule_only_ever_shortens_a_live_row(
        ttl in 0u64..(60 * DAY),
        age in 0u64..(60 * DAY),
        at in proptest::option::of(0u64..(2 * NOW)),
    ) {
        if let Some(cut) = rescheduled_negative_ttl(ttl, age, DAY, at, NOW) {
            proptest::prop_assert!(cut < ttl);
            proptest::prop_assert!(cut > age);
            proptest::prop_assert!(ttl <= DAY);
        }
    }

    #[test]
    fn a_negative_cut_never_comes_before_a_positive_one(
        ttl in 1u64..=DAY,
        age in 0u64..DAY,
        at in 0u64..(2 * NOW),
    ) {
        proptest::prop_assume!(age < ttl);
        // The longer grace: a negative row outlives the drop at least
        // as long as a count does.
        let pos = rescheduled_ttl(ttl, age, DAY, Some(at), NOW).unwrap_or(ttl);
        let neg = rescheduled_negative_ttl(ttl, age, DAY, Some(at), NOW).unwrap_or(ttl);
        proptest::prop_assert!(neg >= pos);
    }
}
