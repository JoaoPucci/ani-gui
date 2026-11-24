//! When a finished show's rows have settled.

use super::*;

/// 2026-06-01, in days since 1970-01-01.
const JUNE_1: i64 = 20_605;

#[test]
fn a_show_finished_a_week_ago_or_more_has_settled() {
    assert!(settled_finish(
        Some("finished"),
        Some("2026-06-01"),
        JUNE_1 + 7
    ));
    assert!(settled_finish(
        Some("finished"),
        Some("2026-06-01"),
        JUNE_1 + 400
    ));
}

#[test]
fn a_show_finished_less_than_a_week_ago_has_not() {
    for after in [-30, 0, 1, 6] {
        assert!(
            !settled_finish(Some("finished"), Some("2026-06-01"), JUNE_1 + after),
            "{after}"
        );
    }
}

#[test]
fn an_end_with_no_day_or_no_end_at_all_has_not() {
    for end in [
        None,
        Some("2026-06"),
        Some(""),
        Some("2026-02-30"),
        Some("soon"),
    ] {
        assert!(
            !settled_finish(Some("finished"), end, JUNE_1 + 400),
            "{end:?}"
        );
    }
}

#[test]
fn a_show_that_has_not_finished_has_not() {
    for status in [
        None,
        Some("current"),
        Some("upcoming"),
        Some("unreleased"),
        Some("tba"),
    ] {
        assert!(
            !settled_finish(status, Some("2026-06-01"), JUNE_1 + 400),
            "{status:?}"
        );
    }
}

/// Days since 1970-01-01 of a Gregorian date, counted out day by day
/// here as an oracle independent of the code under test.
fn civil_days(y: i32, m: u32, d: u32) -> i64 {
    let leap = |y: i32| (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let mut n: i64 = 0;
    for year in 1970..y {
        n += if leap(year) { 366 } else { 365 };
    }
    for month in 1..m {
        n += i64::from(crate::account::episode_map_calendar::days_in_month(
            y, month,
        ));
    }
    n + i64::from(d) - 1
}

#[test]
fn the_oracle_counts_from_the_epoch() {
    assert_eq!(civil_days(1970, 1, 1), 0);
    assert_eq!(civil_days(2026, 6, 1), JUNE_1);
}

proptest::proptest! {
    /// Settling is for good: a show settled on one day is settled on
    /// every later day.
    #[test]
    fn once_settled_a_show_stays_settled(
        y in 1990i32..2100, m in 1u32..=12, d in 1u32..=28,
        today in 0i64..60_000, later in 0i64..5_000,
    ) {
        let end = format!("{y:04}-{m:02}-{d:02}");
        if settled_finish(Some("finished"), Some(&end), today) {
            proptest::prop_assert!(settled_finish(Some("finished"), Some(&end), today + later));
        }
    }

    /// No show has settled within the week after its end date, nor
    /// before it.
    #[test]
    fn no_show_settles_inside_the_week(
        y in 1990i32..2100, m in 1u32..=12, d in 1u32..=28, back in 0i64..7,
    ) {
        let end = format!("{y:04}-{m:02}-{d:02}");
        let ended = civil_days(y, m, d);
        proptest::prop_assert!(!settled_finish(Some("finished"), Some(&end), ended + 6 - back));
        proptest::prop_assert!(settled_finish(Some("finished"), Some(&end), ended + 7));
    }

    /// Only Kitsu's exact `finished` can settle.
    #[test]
    fn only_a_finished_show_settles(status in proptest::option::of("\\PC{0,12}")) {
        let settled = settled_finish(status.as_deref(), Some("2026-06-01"), JUNE_1 + 400);
        proptest::prop_assert_eq!(settled, status.as_deref() == Some("finished"));
    }
}
