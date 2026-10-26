//! Property coverage for the chain rule's month reading: a run is
//! compared by the month of a date only where the date is a real one.

use super::*;
use proptest::prelude::*;

/// Whether `text` is a real calendar date or month, decided here
/// without the calendar module: exactly `YYYY-MM-DD` or `YYYY-MM`, all
/// ASCII digits but the dashes, a month 1–12, and a day the month has
/// in the Gregorian calendar.
fn is_real_date(text: &str) -> bool {
    let b = text.as_bytes();
    let digit = |i: usize| b[i].is_ascii_digit();
    let number =
        |r: std::ops::Range<usize>| b[r].iter().fold(0u32, |n, c| n * 10 + u32::from(c - b'0'));
    let month_ok = (b.len() == 7 || b.len() == 10)
        && (0..4).all(digit)
        && b[4] == b'-'
        && (5..7).all(digit)
        && (1..=12).contains(&number(5..7));
    if !month_ok {
        return false;
    }
    if b.len() == 7 {
        return true;
    }
    if b[7] != b'-' || !(8..10).all(digit) {
        return false;
    }
    let (year, month, day) = (number(0..4), number(5..7), number(8..10));
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let lengths = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let index = usize::try_from(month - 1).unwrap_or(0);
    day >= 1 && day <= lengths[index]
}

/// Text as a date field could carry it: dates and months with every
/// field around its bounds, the end of February across leap and
/// century years, near misses, and anything at all.
fn date_text() -> impl Strategy<Value = String> {
    prop_oneof![
        (0u32..10_000, 0u32..=13, 0u32..=32).prop_map(|(y, m, d)| format!("{y:04}-{m:02}-{d:02}")),
        (0u32..10_000, 0u32..=13).prop_map(|(y, m)| format!("{y:04}-{m:02}")),
        (
            prop::sample::select(vec![1900u32, 2000, 2023, 2024, 2100, 2400]),
            28u32..=30
        )
            .prop_map(|(y, d)| format!("{y:04}-02-{d:02}")),
        "[0-9-]{0,11}",
        ".{0,12}",
    ]
}

proptest! {
    /// A date's month is its first seven characters exactly when the
    /// date reads as a real calendar date or month; any other text, and
    /// no date at all, has no month — and no text panics the read.
    #[test]
    fn a_month_is_read_exactly_from_a_real_date(text in date_text()) {
        let want = is_real_date(&text).then(|| &text[..7]);
        prop_assert_eq!(month(Some(&text)), want);
        prop_assert_eq!(month(None), None);
    }
}
