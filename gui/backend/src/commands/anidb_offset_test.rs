use super::*;

#[test]
fn translation_is_identity_at_offset_zero() {
    assert_eq!(provider_ep_no("5", 0), "5");
    assert_eq!(kitsu_ep_no("5", 0), "5");
}

#[test]
fn non_numeric_episodes_pass_through_both_ways() {
    assert_eq!(provider_ep_no("finale", 40), "finale");
    assert_eq!(kitsu_ep_no("finale", 40), "finale");
}

#[test]
fn numbers_at_or_below_the_offset_are_not_collapsed() {
    // A stored "3" against a stamp of 40 means the stamp doesn't
    // describe this row (stale stamp, foreign writer): serving raw
    // beats serving zero or wrapping.
    assert_eq!(kitsu_ep_no("3", 40), "3");
    assert_eq!(kitsu_ep_no("40", 40), "40");
}

proptest::proptest! {
    // The write and read boundaries are exact inverses over every
    // number a play can produce, whatever the stamped offset.
    #[test]
    fn provider_and_kitsu_translation_round_trip(
        episode in 1u32..=100_000,
        offset in 0u32..=50_000,
    ) {
        let provider = provider_ep_no(&episode.to_string(), offset);
        proptest::prop_assert_eq!(
            kitsu_ep_no(&provider, offset),
            episode.to_string()
        );
    }
}
