//! Property coverage for the chain rule's month reading: a run is
//! compared by the month of a date only where the date is a real one.

use super::*;
use crate::account::episode_map_calendar::parse_date;
use proptest::prelude::*;

/// Text as a date field could carry it: real dates and months, near
/// misses, and anything at all.
fn date_text() -> impl Strategy<Value = String> {
    prop_oneof![
        (1900i32..2100, 1u32..=12, 1u32..=28).prop_map(|(y, m, d)| format!("{y:04}-{m:02}-{d:02}")),
        (1900i32..2100, 1u32..=12).prop_map(|(y, m)| format!("{y:04}-{m:02}")),
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
        let want = parse_date(&text).map(|_| &text[..7]);
        prop_assert_eq!(month(Some(&text)), want);
        prop_assert_eq!(month(None), None);
    }
}
