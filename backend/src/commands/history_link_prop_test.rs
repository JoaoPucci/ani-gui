//! The link plan's properties over arbitrary histories.

use super::*;
use proptest::prelude::*;

/// Row ids as the history holds them: two listings of one show's words
/// on a walked provider, bare rows of a retired one (letter case
/// aside), another show, and an allanime-era id with no words.
const IDS: &[&str] = &[
    "hianime:x-1",
    "hianime:x-2",
    "x-3",
    "X-4",
    "hianime:y-5",
    "y-6",
    "abc",
];

fn id() -> impl Strategy<Value = String> {
    proptest::sample::select(IDS).prop_map(str::to_string)
}

fn entry() -> impl Strategy<Value = HistoryEntry> {
    (
        id(),
        "[0-9]{1,3}",
        proptest::option::of(1_700_000_000_000i64..1_800_000_000_000),
        proptest::option::of("[1-4]"),
    )
        .prop_map(|(id, ep_no, watched_at, kitsu_id)| HistoryEntry {
            ep_no,
            id,
            title: "Show".into(),
            watched_at,
            kitsu_id,
            own_number: false,
        })
}

/// A history: rows with distinct ids, as the writer keeps them.
fn history() -> impl Strategy<Value = Vec<HistoryEntry>> {
    proptest::collection::vec(entry(), 0..6).prop_map(|mut rows| {
        let mut seen = std::collections::HashSet::new();
        rows.retain(|r| seen.insert(r.id.clone()));
        rows
    })
}

proptest! {
    #[test]
    fn a_plan_names_held_rows_of_the_cards_listing_once_in_the_order_asked(
        entries in history(),
        card in id(),
        asked in proptest::collection::vec(id(), 0..8),
    ) {
        let planned = plan_link(&entries, &card, &asked);
        let mut expected: Vec<&String> = Vec::new();
        for a in &asked {
            if entries.iter().any(|e| &e.id == a)
                && card_listing(&card, a)
                && !expected.contains(&a)
            {
                expected.push(a);
            }
        }
        let names: Vec<&String> = planned.iter().map(|p| &p.row).collect();
        prop_assert_eq!(names, expected);
        for p in &planned {
            let e = entries.iter().find(|e| e.id == p.row).unwrap();
            prop_assert_eq!(&p.previous, &e.kitsu_id);
            prop_assert_eq!(&p.ep_no, &e.ep_no);
            prop_assert_eq!(p.watched_at, e.watched_at);
        }
    }

    #[test]
    fn a_row_shares_its_own_listing_and_the_relation_is_symmetric(
        a in "(hianime:)?[a-zA-Z]{1,3}(-[a-z0-9]{1,3}){0,2}(-[0-9]{1,4})?",
        b in "(hianime:)?[a-zA-Z]{1,3}(-[a-z0-9]{1,3}){0,2}(-[0-9]{1,4})?",
    ) {
        prop_assert!(card_listing(&a, &a));
        prop_assert_eq!(card_listing(&a, &b), card_listing(&b, &a));
    }

    #[test]
    fn two_listings_of_a_walked_provider_never_share(
        words in "[a-z]{1,6}(-[a-z0-9]{1,4}){0,3}",
        one in 1u32..100_000,
        other in 1u32..100_000,
        other_words in "[a-z]{1,6}(-[a-z0-9]{1,4}){0,3}",
    ) {
        // JoJo's 2000 OVA and its 2012 series: the same words or not,
        // under another id another listing.
        let card = format!("hianime:{words}-{one}");
        let row = format!("hianime:{other_words}-{other}");
        prop_assume!(card != row);
        prop_assert!(!card_listing(&card, &row));
    }

    #[test]
    fn a_bare_row_shares_a_walked_listing_by_its_words_alone(
        words in "[a-z]{1,6}(-[a-z0-9]{1,4}){0,3}",
        other_words in "[a-z]{1,6}(-[a-z0-9]{1,4}){0,3}",
        upper in any::<bool>(),
        walked in 1u32..100_000,
        bare in 1u32..100_000,
    ) {
        let spelled = if upper { other_words.to_ascii_uppercase() } else { other_words.clone() };
        let card = format!("hianime:{words}-{walked}");
        let row = format!("{spelled}-{bare}");
        prop_assert_eq!(card_listing(&card, &row), words == other_words);
        prop_assert_eq!(card_listing(&row, &card), words == other_words);
    }

    #[test]
    fn a_row_with_no_words_shares_no_other_listing(
        legacy in "[A-Za-z]{4,12}",
        other in "(hianime:)?[a-z]{1,6}(-[a-z0-9]{1,4}){0,2}-[0-9]{1,4}",
    ) {
        prop_assert!(!card_listing(&legacy, &other));
    }
}
