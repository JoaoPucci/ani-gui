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
    /// Every row is exactly one of regular or extra: the two views
    /// partition any listing.
    #[test]
    fn regular_and_extra_rows_partition_the_listing(
        slots in proptest::collection::vec(
            proptest::prop_oneof![
                proptest::strategy::Just(None),
                proptest::strategy::Strategy::prop_map("[0-9]{1,4}", Some),
                proptest::strategy::Strategy::prop_map("[0-9]{1,4}\\.[0-9]", Some),
            ],
            0..24,
        ),
    ) {
        let eps: Vec<EpisodeRef> = slots
            .iter()
            .enumerate()
            .map(|(i, slot)| EpisodeRef {
                id: i as u64,
                number: u32::try_from(i).expect("small index") + 1,
                number2: slot.clone(),
            })
            .collect();
        proptest::prop_assert_eq!(
            regular_episode_count(&eps) as usize + extra_episode_tags(&eps).len(),
            eps.len()
        );
    }
}

#[test]
fn offsets_derive_from_display_identities() {
    // A continuation can carry its cumulative numbering in the tags
    // while the integer slots restart at 1: {number: 1, number2:
    // "41"} is display episode 41. An offset of zero would make the
    // whole cour unplayable — request 1 searches for tag "1" and
    // the cap reads 41.
    let eps = vec![
        EpisodeRef {
            id: 10,
            number: 1,
            number2: Some("41".into()),
        },
        EpisodeRef {
            id: 11,
            number: 2,
            number2: Some("42".into()),
        },
    ];
    assert_eq!(numbering_offset(&eps), 40);
    assert_eq!(kitsu_episode_cap(&eps), Some(2));
}

#[test]
fn fractional_extras_are_advertised_in_per_entry_numbering() {
    // Offset 40: the provider's "41.5" recap is per-entry "1.5" —
    // the numbering every advertised episode uses. Verbatim, the
    // extra falls outside the two-episode cour's page and vanishes.
    let eps = vec![
        EpisodeRef {
            id: 10,
            number: 1,
            number2: Some("41".into()),
        },
        EpisodeRef {
            id: 12,
            number: 2,
            number2: Some("41.5".into()),
        },
        EpisodeRef {
            id: 11,
            number: 3,
            number2: Some("42".into()),
        },
    ];
    assert_eq!(extra_episode_tags(&eps), vec!["1.5".to_string()]);
}

proptest::proptest! {
    /// Advertising and resolving are inverses: a provider fraction
    /// whose integer part sits above the offset translates to
    /// per-entry form and back to exactly itself.
    #[test]
    fn fraction_translation_round_trips(
        n in 1u32..50_000,
        frac in 0u32..10,
        offset in 0u32..40_000,
    ) {
        let provider = format!("{}.{frac}", n + offset);
        let advertised = per_entry_fraction(&provider, offset);
        proptest::prop_assert_eq!(&advertised, &format!("{n}.{frac}"));
        proptest::prop_assert_eq!(provider_fraction(&advertised, offset), provider);
    }
}

#[test]
fn the_cap_counts_display_identities_not_integer_slots() {
    // The recap in slot 3 displays "2.5": the show has two real
    // episodes plus a fractional extra, and a cap of 3 would
    // advertise an episode 3 no request can resolve.
    let eps = vec![
        EpisodeRef {
            id: 1,
            number: 1,
            number2: None,
        },
        EpisodeRef {
            id: 2,
            number: 2,
            number2: None,
        },
        EpisodeRef {
            id: 3,
            number: 3,
            number2: Some("2.5".into()),
        },
    ];
    assert_eq!(kitsu_episode_cap(&eps), Some(2));
}

proptest::proptest! {
    /// Fractional rows are extras, whatever integer slots they
    /// occupy: appending them to any listing leaves the integer cap
    /// exactly where it was.
    #[test]
    fn fractional_rows_never_move_the_cap(
        numbers in proptest::collection::vec(1u32..500, 1..12),
        tags in proptest::collection::vec("[0-9]{1,3}\\.[0-9]", 0..6),
    ) {
        let mut eps = refs(&numbers);
        let base = kitsu_episode_cap(&eps);
        let max = *numbers.iter().max().expect("non-empty");
        for (i, t) in tags.iter().enumerate() {
            eps.push(EpisodeRef {
                id: 900 + i as u64,
                number: max + 1 + u32::try_from(i).expect("small index"),
                number2: Some(t.clone()),
            });
        }
        proptest::prop_assert_eq!(kitsu_episode_cap(&eps), base);
    }
}

#[test]
fn extras_are_the_non_integer_display_tags_in_listing_order() {
    // An integer number2 is a continuation's cumulative re-display,
    // not an extra; every non-integer tag is playable verbatim
    // through the resolve's number2 match and must be advertised.
    let eps = vec![
        EpisodeRef {
            id: 1,
            number: 1,
            number2: None,
        },
        EpisodeRef {
            id: 2,
            number: 2,
            number2: Some("42".into()),
        },
        EpisodeRef {
            id: 3,
            number: 3,
            number2: Some("2.5".into()),
        },
        EpisodeRef {
            id: 4,
            number: 4,
            number2: Some("1061.5".into()),
        },
    ];
    assert_eq!(
        extra_episode_tags(&eps),
        vec!["2.5".to_string(), "1061.5".to_string()]
    );
}

proptest::proptest! {
    /// Interleave integer re-displays, fractional tags, and untagged
    /// rows in any order: the extras are exactly the fractional tags,
    /// in listing order, built here by construction rather than by
    /// re-running the filter under test.
    #[test]
    fn extras_are_exactly_the_fractional_tags(
        slots in proptest::collection::vec(
            proptest::prop_oneof![
                proptest::strategy::Just(None),
                proptest::strategy::Strategy::prop_map(
                    "[0-9]{1,4}", |s| Some((false, s))
                ),
                proptest::strategy::Strategy::prop_map(
                    "[0-9]{1,4}\\.[0-9]", |s| Some((true, s))
                ),
            ],
            0..24,
        ),
    ) {
        let eps: Vec<EpisodeRef> = slots
            .iter()
            .enumerate()
            .map(|(i, slot)| EpisodeRef {
                id: i as u64,
                number: u32::try_from(i).expect("small index") + 1,
                number2: slot.as_ref().map(|(_, s)| s.clone()),
            })
            .collect();
        let expected: Vec<String> = slots
            .iter()
            .flatten()
            .filter(|(fractional, _)| *fractional)
            .map(|(_, s)| s.clone())
            .collect();
        proptest::prop_assert_eq!(extra_episode_tags(&eps), expected);
    }

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
