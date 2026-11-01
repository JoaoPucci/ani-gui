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

/// Repairs from `base` whose hearings all overlap, written in the order
/// given. The first that repairs something is ordered against nothing,
/// and the ones after it overlap it; one that repairs nothing takes no
/// part in the order.
fn overlapping_all(base: &Stamps, repairs: &[&Stamps]) -> Stamps {
    let mut current = base.clone();
    let mut taken = false;
    for ours in repairs {
        if *ours == base {
            continue;
        }
        let order = if taken {
            Order::Overlapping
        } else {
            Order::Later
        };
        current = after(order, &current, base, ours);
        taken = true;
    }
    current
}

fn overlapping(base: &Stamps, first: &Stamps, second: &Stamps) -> Stamps {
    overlapping_all(base, &[first, second])
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
    fn three_overlapping_repairs_end_the_same_in_any_order(
        base in stamps(), a in stamps(), b in stamps(), c in stamps(),
    ) {
        let first = overlapping_all(&base, &[&a, &b, &c]);
        for order in [[&a, &c, &b], [&b, &a, &c], [&b, &c, &a], [&c, &a, &b], [&c, &b, &a]] {
            prop_assert_eq!(&overlapping_all(&base, &order), &first);
        }
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

/// The stamps after repairs heard as given, written in the order given,
/// each making `ours` of whatever stamps it starts from, through
/// [`decided`] and its record of repairs taken.
fn decided_in_turn(base: &Stamps, repairs: &[(Hearing, &Stamps)]) -> Stamps {
    let mut taken = BTreeMap::new();
    let key = (PathBuf::from("history"), "show".to_string());
    let mut current = base.clone();
    for (heard, ours) in repairs {
        if let Some(next) = decided(
            &mut taken,
            key.clone(),
            current.clone(),
            Some(*heard),
            |_| (*ours).clone(),
        ) {
            current = next;
        }
    }
    current
}

/// Stamps with a display stamp: what a show a repair reaches has, and
/// what a repair leaves it.
fn shown() -> impl Strategy<Value = Stamps> {
    stamps().prop_filter("a display stamp", |s| s.display.is_some())
}

proptest! {
    #[test]
    fn two_repairs_one_heard_after_the_other_end_the_same_whichever_writes_first(
        base in shown(), a in shown(), z in shown(),
    ) {
        let (earlier, later) = (
            Hearing { run: 1, start: 0, at: 1 },
            Hearing { run: 1, start: 2, at: 3 },
        );
        let one = decided_in_turn(&base, &[(earlier, &a), (later, &z)]);
        let other = decided_in_turn(&base, &[(later, &z), (earlier, &a)]);
        prop_assert_eq!(&one, &other);
        // The later listing's, unless it repairs nothing.
        let want = if z != base { &z } else if a != base { &a } else { &base };
        prop_assert_eq!(&one, want);
    }

    #[test]
    fn repairs_heard_together_end_the_same_in_any_order(
        base in shown(), a in shown(), b in shown(), c in shown(),
    ) {
        let heard = [0, 1, 2].map(|n| Hearing { run: 1, start: n, at: n + 10 });
        let all = [(heard[0], &a), (heard[1], &b), (heard[2], &c)];
        let first = decided_in_turn(&base, &all);
        for order in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
            prop_assert_eq!(&decided_in_turn(&base, &order.map(|i| all[i])), &first);
        }
    }
}

/// A later listing's repair that read the stamp before another repair
/// here moved it, and reaches the store's locks after: the move was this
/// run's own, so the order decides, and the later listing's stamp is
/// written. (A stamp something else moved meanwhile is left as it is.)
#[test]
fn a_stamp_this_run_s_own_repair_moved_meanwhile_is_ordered_not_left() {
    const SLUG: &str = "hianime:the-show-1";
    let td = tempfile::tempdir().expect("td");
    let mut state = crate::commands::play::tests::state_with_proxy_origin();
    state.history_path = td.path().join("history");
    crate::commands::numbering_offset::put_display(&state, SLUG, 0, 13, "24.5");
    let store = state.history_path.with_file_name("ani-gui-offsets");
    let lock_path = state.history_path.with_file_name("ani-gui-offsets.lock");
    let foreign = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .expect("lock file");
    fs4::FileExt::lock(&foreign).expect("hold the store");
    let queue = crate::held::queue(lock_path.display().to_string());
    let earlier = Hearing {
        run: u64::MAX,
        start: 1,
        at: 2,
    };
    let later = Hearing {
        run: u64::MAX,
        start: 3,
        at: 4,
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let history = state.history_path.clone();
    let late = std::thread::spawn(move || {
        let tags = [(13, "24.5".to_string(), "12.5".to_string())];
        let listing = Listing {
            former_tags: &tags,
            shared: &|_| false,
            rows: None,
        };
        repair(&state, SLUG, &listing, Some(later));
        tx.send(()).ok();
    });
    // The later repair has read the stamp and waits on the store.
    queue.joined_blocking(|| rx.try_recv().is_ok());
    // Meanwhile an earlier listing's repair here moved the stamp.
    let moved = Stamps {
        display: Some((13, "12.75".into())),
        former: None,
    };
    std::fs::write(&store, format!("{SLUG}\t0\t13\t12.75\n")).expect("the earlier repair");
    TAKEN.lock().expect("taken").insert(
        (history, SLUG.to_string()),
        Taken {
            heard: earlier,
            base: Stamps {
                display: Some((13, "24.5".into())),
                former: None,
            },
            left: moved,
        },
    );
    fs4::FileExt::unlock(&foreign).expect("let the store go");
    late.join().expect("the later repair");
    let body = std::fs::read_to_string(&store).expect("store");
    assert_eq!(body, format!("{SLUG}\t0\t13\t12.5\t13\t24.5\n"));
}
