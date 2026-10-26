//! The episode map's dates: real calendar dates or nothing.

use super::*;
use proptest::prelude::*;

#[test]
fn a_full_date_and_a_month_read_as_they_are() {
    assert_eq!(
        parse_date("2020-02-29"),
        Some(CalendarDate {
            year: 2020,
            month: 2,
            day: Some(29)
        })
    );
    assert_eq!(
        parse_date("1989-04"),
        Some(CalendarDate {
            year: 1989,
            month: 4,
            day: None
        })
    );
}

#[test]
fn an_impossible_or_malformed_date_is_no_date() {
    for bad in [
        "2020-02-30",
        "2021-02-29",
        "1900-02-29",
        "2021-04-31",
        "2020-13-01",
        "2020-00-10",
        "2020-10-00",
        "2020-10-32",
        "2020-13",
        "2020-00",
        "2020",
        "",
        "2020-1-011",
        "2020/10/01",
        "20x0-10-01",
        "2020-10-xx",
        "2020-10-1",
        "+020-10-01",
        "2020-10-01T00",
        "２０２０-10-01",
    ] {
        assert_eq!(parse_date(bad), None, "{bad:?}");
    }
    assert_eq!(parse_date("2000-02-29").map(|d| d.day), Some(Some(29)));
}

#[test]
fn months_have_their_calendar_lengths() {
    assert_eq!(
        (1..=12).map(|m| days_in_month(2021, m)).collect::<Vec<_>>(),
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    );
    assert_eq!(days_in_month(2024, 2), 29);
    assert_eq!(days_in_month(2000, 2), 29);
    assert_eq!(days_in_month(1900, 2), 28);
}

proptest! {
    #[test]
    fn a_written_date_reads_back_exactly_when_it_is_a_real_one(
        year in 1900i32..2100, month in 0u32..15, day in 0u32..33
    ) {
        let text = format!("{year:04}-{month:02}-{day:02}");
        let real = (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month);
        let want = real.then_some(CalendarDate { year, month, day: Some(day) });
        prop_assert_eq!(parse_date(&text), want);
        let month_only = format!("{year:04}-{month:02}");
        let want = (1..=12).contains(&month).then_some(CalendarDate { year, month, day: None });
        prop_assert_eq!(parse_date(&month_only), want);
    }
}

/// The Gregorian leap rule, stated on its own: every fourth year, but
/// not a century unless it is a fourth century.
fn is_leap(year: i32) -> bool {
    year.rem_euclid(400) == 0 || (year.rem_euclid(4) == 0 && year.rem_euclid(100) != 0)
}

proptest! {
    /// Every month of every year is 28 to 31 days long, and a year's
    /// twelve months sum to 366 days in a leap year and 365 otherwise.
    #[test]
    fn a_year_s_months_sum_to_its_length(year: i32, month: u32) {
        prop_assert!((28..=31).contains(&days_in_month(year, month)));
        let total: u32 = (1..=12).map(|m| days_in_month(year, m)).sum();
        prop_assert_eq!(total, if is_leap(year) { 366 } else { 365 });
    }

    /// February alone moves with the year: every other month has the
    /// same length in a leap year as in a common one.
    #[test]
    fn only_february_depends_on_the_year(year: i32, month in 1u32..=12) {
        let want = match month {
            2 if is_leap(year) => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        prop_assert_eq!(days_in_month(year, month), want);
    }
}
