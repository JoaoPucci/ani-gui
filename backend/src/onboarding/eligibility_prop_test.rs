//! The eligibility rules' properties over generated registries,
//! records, releases and flag states.

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

use super::super::decl::{Kind, Moment, Surface, TipDecl};
use super::*;

fn release() -> impl Strategy<Value = Release> {
    (0u32..2, 15u32..30, 0u32..4).prop_map(|(a, b, c)| Release::new(a, b, c))
}

prop_compose! {
    fn a_tip(n: usize)(
        since in release(),
        extra in proptest::option::of(1u32..5),
        flagged in any::<bool>(),
        on_demand in proptest::bool::weighted(0.15),
    ) -> TipDecl {
        let until = extra.map(|m| since.plus_minors(m));
        let flag = flagged.then(|| format!("flag_{}", n % 2));
        TipDecl {
            id: format!("tip-{n}"),
            kind: Kind::Single,
            since,
            // A flagged tip always declares its end.
            until: if flag.is_some() { Some(until.unwrap_or(since.plus_minors(4))) } else { until },
            surface: Surface::Show,
            moment: Moment::Arrive,
            flag,
            places: vec![format!("place-{n}")],
            how_to: None,
            on_demand,
        }
    }
}

fn tips() -> impl Strategy<Value = Vec<TipDecl>> {
    (0usize..6).prop_flat_map(|len| (0..len).map(a_tip).collect::<Vec<_>>())
}

prop_compose! {
    fn a_record()(
        installed in proptest::option::of(release()),
        baseline in any::<bool>(),
        seen in proptest::collection::btree_set((0usize..6).prop_map(|n| format!("tip-{n}")), 0..4),
        on0 in proptest::option::of(release()),
        on1 in proptest::option::of(release()),
    ) -> Record {
        let mut flag_on_since = BTreeMap::new();
        if let Some(v) = on0 { flag_on_since.insert("flag_0".to_owned(), v); }
        if let Some(v) = on1 { flag_on_since.insert("flag_1".to_owned(), v); }
        Record { installed_in: installed, baseline_pending: baseline, seen, flag_on_since }
    }
}

fn flags() -> impl Strategy<Value = BTreeSet<String>> {
    proptest::collection::btree_set(
        prop_oneof![Just("flag_0".to_owned()), Just("flag_1".to_owned())],
        0..3,
    )
}

proptest! {
    /// No tip is offered before the release that shipped it or at or
    /// after its declared end, and a flagged one not past two minors
    /// from where its window opened.
    #[test]
    fn a_tip_is_offered_only_inside_its_window(
        tips in tips(), rec in a_record(), current in release(), on in flags(),
    ) {
        let flag_on = |f: &str| on.contains(f);
        for t in &tips {
            if !offered(t, &rec, current, flag_on) { continue; }
            prop_assert!(t.since <= current);
            if let Some(until) = t.until { prop_assert!(current < until); }
            let (opens, end) = window(t, &rec, current, flag_on).expect("offered has a window");
            prop_assert!(opens <= current && current < end);
            if t.flag.is_some() {
                prop_assert!(end <= opens.plus_minors(OFFERED_MINORS));
            }
        }
    }

    /// A flagged tip with its flag on opens at the later of its
    /// shipping release and its flag's first recorded release (this
    /// one, when none is recorded), and ends two minors on, or at its
    /// declared end if that comes first.
    #[test]
    fn a_flagged_window_counts_from_the_flag(
        tips in tips(), rec in a_record(), current in release(), on in flags(),
    ) {
        let flag_on = |f: &str| on.contains(f);
        for t in tips.iter().filter(|t| !t.on_demand) {
            let Some(flag) = t.flag.as_deref() else { continue };
            if !on.contains(flag) { continue; }
            let opens = t.since.max(rec.flag_on_since(flag).unwrap_or(current));
            let end = t.until.expect("a flagged tip declares its end").min(opens.plus_minors(OFFERED_MINORS));
            let want = (opens < end).then_some((opens, end));
            prop_assert_eq!(window(t, &rec, current, flag_on), want);
        }
    }

    /// Without a declared end, an unflagged tip is offered in exactly
    /// the two minors from the release that shipped it. The expected
    /// end is computed with `plus_minors`, whose own values
    /// `release_test.rs` checks by literal.
    #[test]
    fn the_default_window_is_two_minors(since in release(), current in release()) {
        let t = TipDecl {
            id: "t".into(), kind: Kind::Single, since, until: None,
            surface: Surface::Show, moment: Moment::Arrive, flag: None,
            places: vec!["p".into()], how_to: None, on_demand: false,
        };
        let inside = since <= current && current < since.plus_minors(2);
        prop_assert_eq!(offered(&t, &Record::default(), current, |_: &str| false), inside);
    }

    /// A flagged tip is never offered with its flag off, and an
    /// on-demand tip never at all.
    #[test]
    fn off_flags_and_on_demand_tips_are_never_offered(
        tips in tips(), rec in a_record(), current in release(), on in flags(),
    ) {
        let flag_on = |f: &str| on.contains(f);
        for t in &tips {
            let flag_off = t.flag.as_deref().is_some_and(|f| !on.contains(f));
            if t.on_demand || flag_off {
                prop_assert!(!offered(t, &rec, current, flag_on));
            }
        }
    }

    /// Pending is offered and unseen.
    #[test]
    fn pending_is_offered_and_unseen(
        tips in tips(), rec in a_record(), current in release(), on in flags(),
    ) {
        let flag_on = |f: &str| on.contains(f);
        for t in &tips {
            prop_assert_eq!(
                pending(t, &rec, current, flag_on),
                offered(t, &rec, current, flag_on) && !rec.seen.contains(&t.id)
            );
        }
    }

    /// A launch only adds to the record: nothing seen is forgotten, no
    /// flag's first launch moves, and the release a first-ever install
    /// started on stays.
    #[test]
    fn sync_only_adds(tips in tips(), rec in a_record(), current in release(), on in flags()) {
        let next = sync(&tips, &rec, current, |f: &str| on.contains(f));
        prop_assert!(rec.seen.is_subset(&next.seen));
        for (flag, since) in &rec.flag_on_since {
            prop_assert_eq!(next.flag_on_since.get(flag), Some(since));
        }
        prop_assert_eq!(next.installed_in, rec.installed_in);
        prop_assert!(!next.baseline_pending);
    }

    /// Taking the same launch in twice changes nothing more.
    #[test]
    fn sync_is_idempotent(tips in tips(), rec in a_record(), current in release(), on in flags()) {
        let flag_on = |f: &str| on.contains(f);
        let once = sync(&tips, &rec, current, flag_on);
        prop_assert_eq!(sync(&tips, &once, current, flag_on), once);
    }

    /// After a first-ever install's baseline nothing is pending at that
    /// release; after an update's sync, nothing new is seen.
    #[test]
    fn the_baseline_leaves_nothing_pending_and_an_update_marks_nothing(
        tips in tips(), rec in a_record(), current in release(), on in flags(),
    ) {
        let flag_on = |f: &str| on.contains(f);
        let next = sync(&tips, &rec, current, flag_on);
        if rec.baseline_pending {
            for t in &tips {
                prop_assert!(!pending(t, &next, current, flag_on), "{} still pending", t.id);
            }
        } else {
            prop_assert_eq!(&next.seen, &rec.seen);
        }
    }
}
