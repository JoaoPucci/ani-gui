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
