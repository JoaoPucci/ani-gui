//! The release the tips are counted at: the build's own, or a test
//! run's stand-in.

use std::ffi::OsStr;

use super::*;

#[test]
fn the_build_counts_at_its_own_release() {
    assert_eq!(built_version().to_string(), crate::VERSION);
}

#[test]
fn a_test_run_may_name_a_stand_in() {
    assert_eq!(
        test_run_version(Some(OsStr::new("1")), Some("0.19.0")),
        Some(Release::new(0, 19, 0))
    );
}

#[test]
fn outside_a_test_run_the_stand_in_means_nothing() {
    assert_eq!(test_run_version(None, Some("0.19.0")), None);
    assert_eq!(
        test_run_version(Some(OsStr::new("0")), Some("0.19.0")),
        None
    );
    assert_eq!(test_run_version(Some(OsStr::new("")), Some("0.19.0")), None);
}

#[test]
fn a_stand_in_that_is_not_a_release_means_nothing() {
    assert_eq!(test_run_version(Some(OsStr::new("1")), Some("soon")), None);
    assert_eq!(test_run_version(Some(OsStr::new("1")), None), None);
}
