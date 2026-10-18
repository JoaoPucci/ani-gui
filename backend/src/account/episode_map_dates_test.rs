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
