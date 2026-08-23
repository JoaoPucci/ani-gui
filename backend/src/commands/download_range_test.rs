use super::episode_range;

#[test]
fn episode_range_accepts_ordered_integer_pairs_only() {
    // The UI sends "1-12" for All and "s-e" for Range; a start equal
    // to the end is normalized away by the UI but stays a valid pair.
    assert_eq!(episode_range("1-12"), Some((1, 12)));
    assert_eq!(episode_range("3-3"), Some((3, 3)));
    // Singles — integer and fractional — are the other path's job.
    assert_eq!(episode_range("5"), None);
    assert_eq!(episode_range("6.5"), None);
    // A reversed pair is not a range; it falls through to the
    // episode resolver's typed NoResults rather than silently
    // downloading nothing.
    assert_eq!(episode_range("12-1"), None);
    // Fractional halves are not ranges either — the resolver's tag
    // match owns fractional identity.
    assert_eq!(episode_range("1.5-3"), None);
    assert_eq!(episode_range(""), None);
    assert_eq!(episode_range("-"), None);
}

mod props {
    use super::episode_range;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn every_ordered_pair_parses_and_nothing_else_does(a: u32, b: u32) {
            let joined = format!("{a}-{b}");
            let expected = (a <= b).then_some((a, b));
            prop_assert_eq!(episode_range(&joined), expected);
        }

        #[test]
        fn a_bare_integer_is_never_a_range(n: u32) {
            let single = n.to_string();
            prop_assert_eq!(episode_range(&single), None);
        }
    }
}
