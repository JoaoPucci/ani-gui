//! Property coverage for the MAL timestamp parser.
//!
//! Its own file rather than an append to `mal_user_test.rs`, matching
//! the other property modules on this branch: appended blocks collide
//! with every later addition to the same file.

use crate::meta::mal_user_parse::parse_iso8601_to_epoch;

proptest::proptest! {
    /// The same wall clock with an offset names an instant exactly
    /// that offset away from the same wall clock in UTC. Stated over
    /// arbitrary clocks and offsets because the failure this guards
    /// was silent: every MAL row carried the same skew, so nothing
    /// looked wrong until the rail began sorting them against
    /// AniList's true Unix seconds.
    #[test]
    fn an_offset_shifts_the_instant_by_exactly_that_offset(
        hh in 0i64..24,
        mm in 0i64..60,
        ss in 0i64..60,
        oh in 0i64..15,
        om in 0i64..60,
    ) {
        let wall = format!("2026-07-30T{hh:02}:{mm:02}:{ss:02}");
        let utc = parse_iso8601_to_epoch(&format!("{wall}Z"));
        let ahead = parse_iso8601_to_epoch(&format!("{wall}+{oh:02}:{om:02}"));
        let behind = parse_iso8601_to_epoch(&format!("{wall}-{oh:02}:{om:02}"));
        let offset = oh * 3600 + om * 60;

        // Ahead of UTC is EARLIER in absolute terms: 09:00+09:00 is
        // midnight UTC, not nine hours past it.
        proptest::prop_assert_eq!(utc - ahead, offset);
        proptest::prop_assert_eq!(behind - utc, offset);
    }

    /// A suffix that cannot be read means "already UTC" — the same
    /// tolerance the rest of this parser applies, so one odd row is
    /// placed by its wall clock rather than dropped from the list.
    #[test]
    fn an_unreadable_suffix_is_treated_as_utc(suffix in "[a-yA-Y!-/]{0,6}") {
        let wall = "2026-07-30T12:34:56";
        proptest::prop_assert_eq!(
            parse_iso8601_to_epoch(&format!("{wall}{suffix}")),
            parse_iso8601_to_epoch(&format!("{wall}Z")),
        );
    }
}
