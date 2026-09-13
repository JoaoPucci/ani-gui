//! The resume rule's pure halves.

use super::*;

#[test]
fn a_fraction_is_more_progress_than_its_floor_and_a_malformed_row_is_below_any() {
    assert!(progress_of("12.5") > progress_of("12"));
    assert_eq!(progress_of("abc"), -1.0);
    assert_eq!(progress_of(""), -1.0);
    assert!(resumes_over((None, "12.5"), (None, "12")));
    assert!(!resumes_over((None, "abc"), (None, "4")));
    assert!(resumes_over((Some(1), "1"), (None, "10")));
    assert!(!resumes_over((None, "5"), (None, "5")));
}

mod resume_props {
    use super::{progress_of, resumes_over};
    use proptest::prelude::*;

    /// An episode number as the app writes it — a whole number, a
    /// recap's fraction — or as a user may have edited it.
    fn ep_no() -> impl Strategy<Value = String> {
        prop_oneof![
            (1u32..2000).prop_map(|n| n.to_string()),
            (1u32..2000, 1u32..10).prop_map(|(n, f)| format!("{n}.{f}")),
            "[a-z ]{0,6}".prop_map(String::from),
        ]
    }

    fn stamp() -> impl Strategy<Value = Option<i64>> {
        prop::option::of(0i64..10_000)
    }

    proptest! {
        /// A number the text begins with is the progress, exactly;
        /// text that begins with none is below every real episode.
        #[test]
        fn progress_is_the_leading_number_or_below_any(n in 1u32..2000, f in 0u32..10, tail in "[a-z ]{0,4}") {
            let whole = format!("{n}{tail}");
            prop_assert_eq!(progress_of(&whole), f64::from(n));
            let frac = format!("{n}.{f}{tail}");
            prop_assert_eq!(progress_of(&frac), format!("{n}.{f}").parse::<f64>().unwrap());
            prop_assert!(progress_of(&tail) < 1.0);
            prop_assert!(progress_of(&tail) < progress_of(&whole));
        }

        /// The rule, as an order: a stamp beats none, a later stamp
        /// beats an earlier one, equal stamps fall to progress, and
        /// a row never resumes over itself — so of two rows exactly
        /// one resumes over the other unless they are equal on every
        /// count.
        #[test]
        fn one_of_two_rows_resumes_over_the_other_unless_equal(
            a in (stamp(), ep_no()),
            b in (stamp(), ep_no()),
        ) {
            let ab = resumes_over((a.0, &a.1), (b.0, &b.1));
            let ba = resumes_over((b.0, &b.1), (a.0, &a.1));
            prop_assert!(!resumes_over((a.0, &a.1), (a.0, &a.1)));
            let equal = a.0 == b.0 && progress_of(&a.1) == progress_of(&b.1);
            if equal {
                prop_assert!(!ab && !ba);
            } else {
                prop_assert!(ab != ba);
            }
            if a.0 != b.0 {
                prop_assert_eq!(ab, a.0 > b.0, "a stamp beats none, a later stamp an earlier");
            } else {
                prop_assert_eq!(ab, progress_of(&a.1) > progress_of(&b.1));
            }
        }
    }
}
