//! Tests for `crate::commands::numbering_offset::repair`: the order a
//! repair's write is decided in, as properties of [`settle`], and what a
//! listing makes of the stamps, as properties of [`repaired`].

use super::*;
use proptest::prelude::*;

fn stamp() -> impl Strategy<Value = Option<(u32, String)>> {
    prop::option::of((
        1u32..4,
        prop::sample::select(vec!["12.5", "24.5", "13", "14"]),
    ))
    .prop_map(|s| s.map(|(slot, tag)| (slot, tag.to_string())))
}

fn stamps() -> impl Strategy<Value = Stamps> {
    (stamp(), stamp()).prop_map(|(display, former)| Stamps { display, former })
}

/// The stamps after a repair `settle` decides.
fn after(order: Order, current: &Stamps, base: &Stamps, ours: &Stamps) -> Stamps {
    settle(order, current, base, ours.clone()).unwrap_or_else(|| current.clone())
}

/// Two repairs from `base` whose hearings overlap, in the order given:
/// the first this run is ordered against nothing.
fn overlapping(base: &Stamps, first: &Stamps, second: &Stamps) -> Stamps {
    let once = after(Order::Later, base, base, first);
    after(Order::Overlapping, &once, base, second)
}

proptest! {
    #[test]
    fn a_repair_heard_before_the_last_one_writes_nothing(
        current in stamps(), base in stamps(), ours in stamps(),
    ) {
        prop_assert_eq!(settle(Order::Earlier, &current, &base, ours), None);
    }

    #[test]
    fn a_repair_heard_after_the_last_one_leaves_its_own_stamps(
        current in stamps(), base in stamps(), ours in stamps(),
    ) {
        let got = after(Order::Later, &current, &base, &ours);
        if ours == base {
            // It repairs nothing: the last repair stands.
            prop_assert_eq!(got, current);
        } else {
            prop_assert_eq!(got, ours);
        }
    }

    #[test]
    fn two_overlapping_repairs_end_the_same_in_either_order(
        base in stamps(), a in stamps(), b in stamps(),
    ) {
        prop_assert_eq!(overlapping(&base, &a, &b), overlapping(&base, &b, &a));
    }

    #[test]
    fn two_overlapping_repairs_name_no_former_tag_only_one_names(
        base in stamps(), a in stamps(), b in stamps(),
    ) {
        prop_assume!(a != base && b != base);
        let got = overlapping(&base, &a, &b);
        if got != base {
            prop_assert!(got.former.is_none() || (got.former == a.former && got.former == b.former));
        }
    }

    #[test]
    fn a_settled_write_changes_the_stamps(
        order in prop::sample::select(vec![Order::Later, Order::Earlier, Order::Overlapping]),
        current in stamps(), base in stamps(), ours in stamps(),
    ) {
        if let Some(next) = settle(order, &current, &base, ours) {
            prop_assert_ne!(next, current);
        }
    }

    #[test]
    fn a_stamp_named_by_its_former_tag_names_its_tag_now(
        slot in 1u32..4,
        shared in any::<bool>(),
        former in stamp(),
    ) {
        let from = Stamps { display: Some((slot, "24.5".into())), former };
        let tags = [(slot, "24.5".to_string(), "12.5".to_string())];
        let listing = Listing { former_tags: &tags, shared: &|_| shared, rows: None };
        let got = repaired(&from, 0, None, &listing);
        prop_assert_eq!(got.display, Some((slot, "12.5".to_string())));
        prop_assert_eq!(got.former, (!shared).then(|| (slot, "24.5".to_string())));
    }

    #[test]
    fn a_listing_with_nothing_to_say_leaves_the_stamps(from in stamps()) {
        let listing = Listing { former_tags: &[], shared: &|_| false, rows: None };
        prop_assert_eq!(repaired(&from, 0, None, &listing), from);
    }
}

fn hearing() -> impl Strategy<Value = Hearing> {
    (1u64..3, 0u64..20, 0u64..20).prop_map(|(run, a, b)| Hearing {
        run,
        start: a.min(b),
        at: a.max(b) + 1,
    })
}

proptest! {
    #[test]
    fn one_hearing_is_later_exactly_when_the_other_is_earlier(a in hearing(), b in hearing()) {
        prop_assume!(a.run == b.run);
        prop_assert_eq!(a.against(Some(b)) == Order::Later, b.against(Some(a)) == Order::Earlier);
        prop_assert_eq!(
            a.against(Some(b)) == Order::Overlapping,
            b.against(Some(a)) == Order::Overlapping
        );
    }

    #[test]
    fn a_hearing_against_none_or_another_run_s_is_later(a in hearing(), b in hearing()) {
        prop_assert_eq!(a.against(None), Order::Later);
        prop_assume!(a.run != b.run);
        prop_assert_eq!(a.against(Some(b)), Order::Later);
    }
}
