//! Which id a row records because the user named it: a refused watch
//! never takes that one away.

use crate::commands::linked_note::stands;
use proptest::prelude::*;

#[test]
fn the_id_the_user_linked_stands() {
    assert!(stands(Some("44294"), Some("44294")));
    assert!(stands(Some("kitsu:44294"), Some("44294")));
}

#[test]
fn another_id_or_none_does_not() {
    assert!(!stands(Some("45412"), Some("44294")));
    assert!(!stands(None, Some("44294")));
    assert!(!stands(Some("44294"), None));
    assert!(!stands(None, None));
}

proptest! {
    #[test]
    fn stands_exactly_when_the_row_records_the_linked_id(
        previous in prop::option::of("[0-9]{1,3}|[a-z]{1,3}"),
        linked in prop::option::of("[0-9]{1,3}|[a-z]{1,3}"),
    ) {
        let want = match (previous.as_deref(), linked.as_deref()) {
            (Some(p), Some(l)) => crate::history::same_kitsu_id(p, l),
            _ => false,
        };
        prop_assert_eq!(stands(previous.as_deref(), linked.as_deref()), want);
    }
}
