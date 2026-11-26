//! A release number's parsing, order and the minor-release count the
//! tips' windows use.

use proptest::prelude::*;

use super::*;

#[test]
fn three_numbers_are_a_release() {
    assert_eq!(Release::parse("0.19.0"), Some(Release::new(0, 19, 0)));
    assert_eq!(Release::parse("v1.2.3"), Some(Release::new(1, 2, 3)));
    assert_eq!(Release::parse(" 0.19.2 "), Some(Release::new(0, 19, 2)));
}

#[test]
fn a_suffix_does_not_count() {
    assert_eq!(Release::parse("0.19.0-dev"), Some(Release::new(0, 19, 0)));
    assert_eq!(Release::parse("0.19.0-rc.1"), Some(Release::new(0, 19, 0)));
    assert_eq!(Release::parse("0.19.0+abc"), Some(Release::new(0, 19, 0)));
}

#[test]
fn anything_else_is_not_a_release() {
    for text in [
        "", "0.19", "0.19.", "0.19.0.1", "a.b.c", "0.-1.0", "0..0", "0.19.x", "v", "1.2.3a",
        " . . ",
    ] {
        assert_eq!(Release::parse(text), None, "{text:?}");
    }
}

#[test]
fn two_minors_after_a_patch_release_is_the_minor_two_on() {
    assert_eq!(
        Release::new(0, 19, 4).plus_minors(2),
        Release::new(0, 21, 0)
    );
    assert_eq!(
        Release::new(0, 19, 0).plus_minors(2),
        Release::new(0, 21, 0)
    );
}

#[test]
fn it_reads_and_writes_as_its_text() {
    let r: Release = serde_json::from_str("\"0.19.0\"").expect("a release");
    assert_eq!(r, Release::new(0, 19, 0));
    assert_eq!(serde_json::to_string(&r).expect("serializes"), "\"0.19.0\"");
    assert!(serde_json::from_str::<Release>("\"soon\"").is_err());
}

proptest! {
    /// What a release prints parses back to it.
    #[test]
    fn printing_round_trips(a in 0u32..1000, b in 0u32..1000, c in 0u32..1000) {
        let r = Release::new(a, b, c);
        prop_assert_eq!(Release::parse(&r.to_string()), Some(r));
    }

    /// Order is major, then minor, then patch.
    #[test]
    fn order_is_by_number(
        a in (0u32..5, 0u32..30, 0u32..10),
        b in (0u32..5, 0u32..30, 0u32..10),
    ) {
        let (x, y) = (Release::new(a.0, a.1, a.2), Release::new(b.0, b.1, b.2));
        prop_assert_eq!(x.cmp(&y), a.cmp(&b));
    }

    /// The release two minors on is later than every release of the
    /// minor it counts from and of the next one, and no later than the
    /// first release of the minor after that.
    #[test]
    fn two_minors_on_ends_the_next_minor(
        major in 0u32..5, minor in 0u32..30, patch in 0u32..10, later in 0u32..10,
    ) {
        let end = Release::new(major, minor, patch).plus_minors(2);
        prop_assert!(Release::new(major, minor, later) < end);
        prop_assert!(Release::new(major, minor + 1, later) < end);
        prop_assert_eq!(end, Release::new(major, minor + 2, 0));
    }
}
