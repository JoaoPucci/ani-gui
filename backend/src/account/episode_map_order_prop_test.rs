//! Properties of the order the chain and split-parts rules read: a run
//! is settled as having started first exactly as the model, reading
//! dates by its own byte layout, says; never against itself; and never
//! both ways.
//!
//! Every strategy function returns a `BoxedStrategy`. Composed without
//! boxing, the nested combinator types grow into symbol names tens of
//! thousands of characters long, and the Windows test build's debug info
//! hits the linker's PDB limit (LNK4319). Box each new strategy too.

use super::super::prop_tests::model;
use super::{parts_in_airing_order, started_before};
use crate::account::episode_map::KitsuShow;
use proptest::prelude::*;

/// A start date as a source could send it, in a two-year window so
/// months and days meet often: whole days, months alone, a year alone,
/// impossible dates and noise, or none.
fn start() -> BoxedStrategy<Option<String>> {
    prop_oneof![
        1 => Just(None),
        6 => model::real_date_in(2020..=2021).prop_map(|d| Some(model::text(d))),
        3 => (2020i32..=2021, 1u32..=12).prop_map(|(y, m)| Some(format!("{y:04}-{m:02}"))),
        1 => (2020i32..=2021).prop_map(|y| Some(format!("{y:04}"))),
        1 => Just(Some("2020-02-30".to_string())),
        1 => "[0-9-]{0,11}".prop_map(Some),
    ]
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// The rule's order is the model's.
    #[test]
    fn a_start_is_first_exactly_as_the_model_reads_it(a in start(), b in start()) {
        prop_assert_eq!(
            started_before(a.as_deref(), b.as_deref()),
            model::aired_first(a.as_deref(), b.as_deref())
        );
    }

    /// No start is before itself, and no two starts are each before the
    /// other.
    #[test]
    fn the_order_is_strict(a in start(), b in start()) {
        prop_assert!(!started_before(a.as_deref(), a.as_deref()));
        prop_assert!(
            !(started_before(a.as_deref(), b.as_deref())
                && started_before(b.as_deref(), a.as_deref()))
        );
    }

    /// The parts in airing order are the longest prefix of the chain in
    /// which no part is settled, by the model, as having started before
    /// the one it follows; every part when none is, and at least the
    /// first.
    #[test]
    fn the_parts_end_before_the_first_that_aired_first(
        starts in proptest::collection::vec(start(), 0..6)
    ) {
        let parts: Vec<KitsuShow> = starts
            .iter()
            .enumerate()
            .map(|(i, s)| KitsuShow {
                id: i.to_string(),
                episodes: Some(1),
                start: s.clone(),
                end: None,
            })
            .collect();
        let want = (1..parts.len())
            .find(|&i| model::aired_first(starts[i].as_deref(), starts[i - 1].as_deref()))
            .unwrap_or(parts.len());
        let got = parts_in_airing_order(&parts);
        prop_assert_eq!(got, want);
        prop_assert!(got >= parts.len().min(1));
    }
}
