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
