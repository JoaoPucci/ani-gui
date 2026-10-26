//! The dates the chain rule compares, at their edges.

use super::*;

#[test]
fn a_full_date_counts_days_across_months_and_years() {
    assert_eq!(days(Some("1970-01-01")), Some(0));
    assert_eq!(
        days(Some("2020-10-01"))
            .zip(days(Some("2020-09-30")))
            .map(|(a, b)| a - b),
        Some(1)
    );
    assert_eq!(
        days(Some("2021-01-01"))
            .zip(days(Some("2020-12-31")))
            .map(|(a, b)| a - b),
        Some(1)
    );
    assert_eq!(
        days(Some("2020-03-01"))
            .zip(days(Some("2020-02-28")))
            .map(|(a, b)| a - b),
        Some(2)
    );
}

#[test]
fn anything_short_of_a_full_date_counts_no_days() {
    for bad in [
        "2020-10",
        "2020",
        "2020/10/01",
        "2020-1-011",
        "2020-13-01",
        "2020-00-10",
        "2020-10-32",
        "20x0-10-01",
    ] {
        assert_eq!(days(Some(bad)), None, "{bad}");
    }
    assert_eq!(days(None), None);
    assert!(!within_slack(Some("2020-09"), Some("2020-10-01")));
}

#[test]
fn slack_is_two_days_and_only_forwards() {
    assert!(within_slack(Some("2020-09-30"), Some("2020-10-02")));
    assert!(!within_slack(Some("2020-09-30"), Some("2020-10-03")));
    assert!(within_slack(Some("2020-10-03"), Some("2020-09-30")));
}

#[test]
fn an_impossible_date_counts_no_days_and_no_slack() {
    // An April 31st or a February 30th is no date: it is never carried
    // into the next month, where it would land within slack of one.
    for bad in ["2020-02-30", "2021-02-29", "2021-04-31", "1900-02-29"] {
        assert_eq!(days(Some(bad)), None, "{bad}");
    }
    assert!(!within_slack(Some("2020-02-29"), Some("2020-02-31")));
    assert!(!within_slack(Some("2021-04-30"), Some("2021-04-31")));
}

proptest::proptest! {
    #[test]
    fn days_are_counted_for_real_dates_alone_and_one_apart(
        year in 1950i32..2050, month in 0u32..14, day in 0u32..33
    ) {
        let text = format!("{year:04}-{month:02}-{day:02}");
        let real = crate::account::episode_map_calendar::parse_date(&text).is_some();
        proptest::prop_assert_eq!(days(Some(&text)).is_some(), real);
        if real {
            // The next real date is one day on.
            let next = if day < crate::account::episode_map_calendar::days_in_month(year, month) {
                format!("{year:04}-{month:02}-{:02}", day + 1)
            } else if month < 12 {
                format!("{year:04}-{:02}-01", month + 1)
            } else {
                format!("{:04}-01-01", year + 1)
            };
            proptest::prop_assert_eq!(days(Some(&next)).zip(days(Some(&text))).map(|(b, a)| b - a), Some(1));
        }
    }
}

use crate::account::episode_map_calendar::{days_in_month, parse_date};
use proptest::prelude::*;

/// The real date one day after `(year, month, day)`, by the same
/// construction the day-count property above steps with.
fn next_day((year, month, day): (i32, u32, u32)) -> (i32, u32, u32) {
    if day < days_in_month(year, month) {
        (year, month, day + 1)
    } else if month < 12 {
        (year, month + 1, 1)
    } else {
        (year + 1, 1, 1)
    }
}

fn text((year, month, day): (i32, u32, u32)) -> String {
    format!("{year:04}-{month:02}-{day:02}")
}

/// Days since 1900-01-01, counted month by month: a model independent
/// of the civil-days formula [`days`] uses.
fn ordinal((year, month, day): (i32, u32, u32)) -> i64 {
    let years: i64 = (1900..year)
        .map(|y| if days_in_month(y, 2) == 29 { 366 } else { 365 })
        .sum();
    let months: i64 = (1..month).map(|m| i64::from(days_in_month(year, m))).sum();
    years + months + i64::from(day)
}

/// A real full date, any day of its month.
fn real_date() -> impl Strategy<Value = (i32, u32, u32)> {
    (1950i32..2050, 1u32..=12).prop_flat_map(|(y, m)| (Just(y), Just(m), 1..=days_in_month(y, m)))
}

/// Text that is no full real date: a month, a year, an impossible day,
/// or anything else that does not read as a day of the calendar.
fn no_full_date() -> impl Strategy<Value = String> {
    prop_oneof![
        (1950i32..2050, 1u32..=12).prop_map(|(y, m)| format!("{y:04}-{m:02}")),
        (1950i32..2050).prop_map(|y| format!("{y:04}")),
        (1950i32..2050, 1u32..=12)
            .prop_map(|(y, m)| format!("{y:04}-{m:02}-{:02}", days_in_month(y, m) + 1)),
        "[0-9-]{0,12}",
    ]
    .prop_filter("a full real date", |t| {
        parse_date(t).and_then(|d| d.day).is_none()
    })
}

proptest! {
    /// Between two real dates the slack holds exactly when the later
    /// is at most two days after the earlier — a later date that comes
    /// before the earlier one is within it too, however far back.
    #[test]
    fn slack_holds_exactly_when_the_later_date_is_at_most_two_days_on(
        earlier in real_date(), later in real_date()
    ) {
        prop_assert_eq!(
            within_slack(Some(&text(earlier)), Some(&text(later))),
            ordinal(later) - ordinal(earlier) <= DAYS_OF_SLACK
        );
    }

    /// A date short of a day, or naming no real day, or none at all,
    /// is within no slack of anything, on either side.
    #[test]
    fn a_partial_or_unreal_date_is_within_no_slack(
        real in real_date(), bad in no_full_date(), other in no_full_date()
    ) {
        let real = text(real);
        prop_assert!(!within_slack(Some(&bad), Some(&real)));
        prop_assert!(!within_slack(Some(&real), Some(&bad)));
        prop_assert!(!within_slack(Some(&bad), Some(&other)));
        prop_assert!(!within_slack(None, Some(&real)));
        prop_assert!(!within_slack(Some(&real), None));
    }

    /// A date `n` days on from another is within its slack exactly for
    /// `n` up to two, and the earlier is always within the later's.
    #[test]
    fn a_date_n_days_on_is_within_slack_exactly_up_to_two(
        start in real_date(), n in 0u32..=7
    ) {
        let mut later = start;
        for _ in 0..n {
            later = next_day(later);
        }
        let (a, b) = (text(start), text(later));
        prop_assert_eq!(within_slack(Some(&a), Some(&b)), i64::from(n) <= DAYS_OF_SLACK);
        prop_assert!(within_slack(Some(&b), Some(&a)));
    }
}
