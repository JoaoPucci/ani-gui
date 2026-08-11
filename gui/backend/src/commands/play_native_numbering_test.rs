use super::*;
use crate::scraper::anidb::EpisodeRef;

fn refs(numbers: &[u32]) -> Vec<EpisodeRef> {
    numbers
        .iter()
        .enumerate()
        .map(|(i, n)| EpisodeRef {
            id: i as u64,
            number: *n,
            number2: None,
        })
        .collect()
}

proptest::proptest! {
    /// Over arbitrary episode vectors — any order, duplicates, gaps,
    /// extremes: an entry whose first listed number is above 1 is a
    /// continuation and shifts by first-1; per-entry listings (a 0
    /// or 1 anywhere) shift nothing; the cap is the highest listed
    /// number in per-entry terms, at least 1 for a continuation's
    /// own first episode, and None exactly for an empty listing.
    #[test]
    fn offset_and_cap_hold_over_arbitrary_listings(
        numbers in proptest::collection::vec(0u32..100_000, 0..24),
    ) {
        let eps = refs(&numbers);
        let offset = numbering_offset(&eps);
        let cap = kitsu_episode_cap(&eps);
        match numbers.iter().min() {
            None => {
                proptest::prop_assert_eq!(offset, 0);
                proptest::prop_assert_eq!(cap, None);
            }
            Some(&min) => {
                if min > 1 {
                    proptest::prop_assert_eq!(offset, min - 1);
                } else {
                    proptest::prop_assert_eq!(offset, 0);
                }
                let max = *numbers.iter().max().expect("non-empty");
                proptest::prop_assert_eq!(cap, Some(max - offset));
                // The listing's own first episode never collapses to
                // zero and the cap covers every listed episode.
                proptest::prop_assert!(min - offset <= 1);
                proptest::prop_assert!(cap.expect("non-empty") >= min - offset);
            }
        }
    }

    /// Translation invariance — shifting a per-entry listing into a
    /// continuation's cumulative numbering moves the offset by
    /// exactly the shift and leaves the per-entry cap unchanged:
    /// TYBW part four listing 41..42 caps at 2, like 1..2 would.
    #[test]
    fn a_shifted_listing_keeps_its_per_entry_cap(
        base in proptest::collection::vec(1u32..500, 1..24),
        shift in 1u32..40_000,
    ) {
        // Anchor the base listing at 1 so the shifted copy's first
        // number is exactly shift+1 — a genuine continuation shape.
        let mut base = base;
        base.push(1);
        let shifted: Vec<u32> = base.iter().map(|n| n + shift).collect();
        let base_eps = refs(&base);
        let shifted_eps = refs(&shifted);
        proptest::prop_assert_eq!(numbering_offset(&base_eps), 0);
        proptest::prop_assert_eq!(numbering_offset(&shifted_eps), shift);
        proptest::prop_assert_eq!(
            kitsu_episode_cap(&shifted_eps),
            kitsu_episode_cap(&base_eps)
        );
    }
}
