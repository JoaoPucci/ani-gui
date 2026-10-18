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
