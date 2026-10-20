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
