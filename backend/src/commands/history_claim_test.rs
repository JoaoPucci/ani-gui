//! Properties of which Kitsu id a history row names.

use super::*;
use proptest::prelude::*;

fn id() -> impl Strategy<Value = String> {
    "[1-4]"
}

fn side() -> impl Strategy<Value = Option<(String, bool)>> {
    proptest::option::of((id(), any::<bool>()))
}

fn as_ref(o: &Option<(String, bool)>) -> Option<(&str, bool)> {
    o.as_ref().map(|(i, g)| (i.as_str(), *g))
}

proptest! {
    #[test]
    fn a_gone_id_is_named_by_no_row(
        asked in id(),
        recorded in side(),
        mapped in side(),
        titled in proptest::collection::vec(id(), 0..3),
    ) {
        let row = RowIds { recorded: as_ref(&recorded), mapped: as_ref(&mapped), title_matched: &titled };
        prop_assert!(!names(&asked, true, &row));
    }

    #[test]
    fn a_standing_recorded_id_decides_alone(
        asked in id(),
        recorded in id(),
        mapped in side(),
        titled in proptest::collection::vec(id(), 0..3),
    ) {
        let row = RowIds { recorded: Some((&recorded, false)), mapped: as_ref(&mapped), title_matched: &titled };
        prop_assert_eq!(names(&asked, false, &row), asked == recorded);
    }

    #[test]
    fn a_row_that_never_recorded_goes_by_its_standing_mapping_alone(
        asked in id(),
        mapped in side(),
        titled in proptest::collection::vec(id(), 0..3),
    ) {
        let row = RowIds { recorded: None, mapped: as_ref(&mapped), title_matched: &titled };
        let expected = matches!(&mapped, Some((m, false)) if *m == asked);
        prop_assert_eq!(names(&asked, false, &row), expected);
    }

    #[test]
    fn a_gone_recorded_id_goes_by_the_mapping_then_the_title_match(
        asked in id(),
        recorded in id(),
        mapped in side(),
        titled in proptest::collection::vec(id(), 0..3),
    ) {
        let row = RowIds { recorded: Some((&recorded, true)), mapped: as_ref(&mapped), title_matched: &titled };
        let expected = match &mapped {
            Some((m, false)) => *m == asked,
            _ => titled.contains(&asked),
        };
        prop_assert_eq!(names(&asked, false, &row), expected);
    }
}
