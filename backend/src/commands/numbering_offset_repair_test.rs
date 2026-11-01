//! Tests for `crate::commands::numbering_offset::repair`: which repairs'
//! stamps stand, as properties of [`standing`] and of [`decided`] with
//! its record of repairs taken, and what a listing makes of the stamps,
//! as properties of [`repaired`].

use super::*;
use crate::commands::listing_tags::Order;
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

fn hearing_of(n: u64, len: u64) -> Hearing {
    Hearing {
        run: 1,
        start: n,
        at: n + len,
    }
}

proptest! {
    #[test]
    fn a_repair_heard_wholly_after_the_others_stands(
        base in stamps(), a in stamps(), b in stamps(), last in stamps(),
    ) {
        let repairs = [(hearing_of(0, 3), a), (hearing_of(1, 3), b), (hearing_of(10, 1), last.clone())];
        prop_assert_eq!(standing(&base, &repairs), last);
    }

    #[test]
    fn a_repair_heard_before_another_has_no_say(
        base in stamps(), early in stamps(), late in stamps(),
    ) {
        let with = [(hearing_of(0, 1), early), (hearing_of(5, 1), late.clone())];
        prop_assert_eq!(standing(&base, &with), standing(&base, &[(hearing_of(5, 1), late)]));
    }

    #[test]
    fn the_order_repairs_were_taken_in_plays_no_part(
        base in stamps(),
        repairs in prop::collection::vec(((0u64..6, 1u64..4), stamps()), 0..4),
    ) {
        let repairs: Vec<(Hearing, Stamps)> =
            repairs.into_iter().map(|((n, len), s)| (hearing_of(n, len), s)).collect();
        let mut reversed = repairs.clone();
        reversed.reverse();
        prop_assert_eq!(standing(&base, &repairs), standing(&base, &reversed));
    }

    #[test]
    fn overlapping_repairs_name_no_former_tag_only_some_name(
        base in stamps(), a in stamps(), b in stamps(),
    ) {
        let got = standing(&base, &[(hearing_of(0, 5), a.clone()), (hearing_of(1, 5), b.clone())]);
        if got != base {
            prop_assert_eq!(&got.display, &a.display);
            prop_assert!(got.former.is_none() || (got.former == a.former && got.former == b.former));
        }
    }

    #[test]
    fn overlapping_repairs_that_put_the_stamp_apart_leave_it_as_it_was(
        base in stamps(), a in stamps(), b in stamps(),
    ) {
        prop_assume!(a.display != b.display);
        let got = standing(&base, &[(hearing_of(0, 5), a), (hearing_of(1, 5), b)]);
        prop_assert_eq!(got, base);
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
        Taken::one(
            earlier,
            Stamps {
                display: Some((13, "24.5".into())),
                former: None,
            },
            moved,
        ),
    );
    fs4::FileExt::unlock(&foreign).expect("let the store go");
    late.join().expect("the later repair");
    let body = std::fs::read_to_string(&store).expect("store");
    assert_eq!(body, format!("{SLUG}\t0\t13\t12.5\t13\t24.5\n"));
}

proptest! {
    #[test]
    fn three_repairs_however_heard_end_the_same_in_any_order(
        base in shown(), a in shown(), b in shown(), c in shown(),
        ha in hearing(), hb in hearing(), hc in hearing(),
    ) {
        let one_run = |h: Hearing| Hearing { run: 1, ..h };
        let all = [(one_run(ha), &a), (one_run(hb), &b), (one_run(hc), &c)];
        let first = decided_in_turn(&base, &all);
        for order in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
            prop_assert_eq!(&decided_in_turn(&base, &order.map(|i| all[i])), &first);
        }
    }
}

fn tag() -> impl Strategy<Value = String> {
    prop::sample::select(vec!["12.5", "24.5", "13", "1.5", "2.50"]).prop_map(str::to_string)
}

/// The stamps after repairs heard as given, each from a listing that
/// carries `carrier`, written in the order given through [`settled`].
fn settled_in_turn(base: &Stamps, repairs: &[(Hearing, &Stamps, &Carrier)]) -> Stamps {
    let mut taken = BTreeMap::new();
    let key = (PathBuf::from("history"), "show".to_string());
    let mut current = base.clone();
    for (heard, ours, carrier) in repairs {
        current = settled(
            &mut taken,
            key.clone(),
            &current,
            Some(*heard),
            carrier,
            |_| (*ours).clone(),
        );
    }
    current
}

proptest! {
    #[test]
    fn a_listing_s_carrier_carries_what_its_rows_tag(
        rows in prop::collection::vec(prop::option::of(tag()), 0..5),
        former in tag(),
    ) {
        let carrier = Carrier::Tags(rows.iter().flatten().cloned().collect());
        let by_rows = rows.iter().flatten().any(|t| crate::commands::play_native_episode::tag_matches(t, &former));
        prop_assert_eq!(carrier.carries(&former), by_rows);
    }

    #[test]
    fn a_resolve_s_carrier_carries_what_its_extra_tags_carry_in_the_entry_s_numbering(
        extra in prop::collection::vec(tag(), 0..4),
        offset in 0u32..13,
        former in tag(),
    ) {
        let carrier = Carrier::Entry { extra: extra.clone(), offset };
        let own = crate::commands::numbering_offset::kitsu_ep_no(&former, offset);
        prop_assert_eq!(
            carrier.carries(&former),
            crate::commands::play_native_merge::carries_tag(&extra, &own)
        );
    }

    #[test]
    fn repairs_and_listings_that_carry_a_tag_end_the_same_in_any_order(
        base in shown(), a in shown(), b in shown(), c in shown(),
        ha in hearing(), hb in hearing(), hc in hearing(),
        ca in prop::collection::vec(tag(), 0..2),
        cb in prop::collection::vec(tag(), 0..2),
        cc in prop::collection::vec(tag(), 0..2),
    ) {
        let one_run = |h: Hearing| Hearing { run: 1, ..h };
        let (ca, cb, cc) = (Carrier::Tags(ca), Carrier::Tags(cb), Carrier::Tags(cc));
        let all = [(one_run(ha), &a, &ca), (one_run(hb), &b, &cb), (one_run(hc), &c, &cc)];
        let first = settled_in_turn(&base, &all);
        for order in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
            prop_assert_eq!(&settled_in_turn(&base, &order.map(|i| all[i])), &first);
        }
        // No listing's carried tag is kept as the former tag.
        if let Some((_, f)) = &first.former {
            prop_assert!(![&ca, &cb, &cc].iter().any(|c| c.carries(f)));
        }
    }
}
