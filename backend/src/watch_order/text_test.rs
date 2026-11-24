//! How the watch order reads text the way the renderer's JavaScript did:
//! a history row's episode (`parseHistoryEpisode`'s `\s`) and an id's
//! number (`Number(id)`), and that ordering by start never breaks on an
//! id that is not a number.

use proptest::prelude::*;

use super::by_start::{by_start, sort_by_start};
use super::fixtures::{entry, ids};
use super::progress::history_episode;

#[test]
fn a_rows_episode_skips_the_whitespace_javascript_skips_and_no_other() {
    // U+FEFF is whitespace to JavaScript's `\s`, not to Rust; U+0085 the reverse.
    assert_eq!(history_episode("\u{feff}7"), Some(7.0));
    assert_eq!(history_episode("\u{a0}\u{3000}7"), Some(7.0));
    assert_eq!(history_episode("\u{85}7"), None);
}

#[test]
fn an_ids_number_is_the_one_javascript_reads() {
    let order = |a: &str, b: &str| by_start(&entry(a, None), &entry(b, None));
    // `Number("")` is 0, `Number(" 5")` is 5, `Number("0x1A")` is 26.
    assert_eq!(order("", "1"), std::cmp::Ordering::Less);
    assert_eq!(order(" 5", "6"), std::cmp::Ordering::Less);
    assert_eq!(order("0x1A", "25"), std::cmp::Ordering::Greater);
    assert_eq!(order("0x1A", "27"), std::cmp::Ordering::Less);
}

#[test]
fn ids_that_are_not_numbers_sort_after_the_ones_that_are() {
    let mut list = vec![
        entry("b", None),
        entry("2", None),
        entry("a", None),
        entry("inf", None),
        entry("1", None),
    ];
    sort_by_start(&mut list);
    assert_eq!(ids(&list), ["1", "2", "a", "b", "inf"]);
}

proptest! {
    #[test]
    fn ordering_by_start_is_a_total_order_whatever_the_ids(
        list in prop::collection::vec(
            (
                prop_oneof![
                    (0u32..50).prop_map(|n| n.to_string()),
                    prop::sample::select(&["", " 3", "0x10", "x", "NaN", "inf", "1e1", "-0"][..]).prop_map(String::from),
                ],
                prop::option::of(prop::sample::select(&["2001-01-01", "2002-01-01"][..])),
            ),
            0..12,
        )
    ) {
        let entries: Vec<_> = list.iter().map(|(id, start)| entry(id, *start)).collect();
        for a in &entries {
            prop_assert_eq!(by_start(a, a), std::cmp::Ordering::Equal);
            for b in &entries {
                prop_assert_eq!(by_start(a, b), by_start(b, a).reverse());
                for c in &entries {
                    if by_start(a, b).is_le() && by_start(b, c).is_le() {
                        prop_assert!(by_start(a, c).is_le());
                    }
                }
            }
        }
        let mut sorted = entries.clone();
        sort_by_start(&mut sorted);
        prop_assert_eq!(sorted.len(), entries.len());
    }
}
