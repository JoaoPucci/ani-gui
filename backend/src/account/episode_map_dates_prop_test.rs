//! Property coverage for whether a chain segment's run lies inside
//! Kitsu's: which bounds the dates leave unplaced, and how Kitsu's own
//! episode dates vouch for them.

use std::collections::{BTreeMap, BTreeSet};

use super::super::prop_tests::model::{self, ordinal, text, Ymd};
use super::*;
use crate::account::episode_map::KitsuShow;
use proptest::prelude::*;

/// A real full date over two years, so months coincide often.
fn real_date() -> impl Strategy<Value = Ymd> {
    model::real_date_in(2020..=2021)
}

/// A date field as a source could send it: mostly real — often a few
/// days from a month boundary — or a month alone, so that all four
/// dates of a run are read often; sometimes impossible, anything at
/// all, or absent.
fn any_date() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        1 => Just(None),
        3 => real_date().prop_map(|d| Some(text(d))),
        3 => model::edge_date_in(2020..=2021).prop_map(|d| Some(text(d))),
        2 => (2020i32..=2021, 1u32..=12).prop_map(|(y, m)| Some(format!("{y:04}-{m:02}"))),
        1 => Just(Some("2020-02-30".to_string())),
        1 => Just(Some("2021-13-01".to_string())),
        1 => Just(Some("2020".to_string())),
        1 => "[0-9-]{0,11}".prop_map(Some),
    ]
}

/// Kitsu air dates for a few episode numbers, real or not.
fn any_airdates() -> impl Strategy<Value = BTreeMap<u32, String>> {
    prop::collection::btree_map(1u32..=4, any_date().prop_filter_map("a date", |d| d), 0..5)
}

fn segment(from: u32, to: u32) -> Segment {
    Segment {
        entry: 1,
        entry_total: to.saturating_sub(from) + 1,
        entry_finished: false,
        kitsu_from: from,
        kitsu_to: to,
        lead: 0,
        extras_after: Vec::new(),
    }
}

fn evidence(
    k_start: Option<String>,
    k_end: Option<String>,
    kitsu_airdates: BTreeMap<u32, String>,
) -> Evidence {
    Evidence {
        kitsu: KitsuShow {
            id: "k".into(),
            start: k_start,
            end: k_end,
            ..Default::default()
        },
        kitsu_airdates,
        ..Default::default()
    }
}

fn tracker(start: Option<String>, end: Option<String>) -> TrackerShow {
    TrackerShow {
        id: 1,
        start,
        end,
        ..Default::default()
    }
}

/// Everything the dates rule reads: Kitsu's run and air dates, the
/// show's run, and the segment's first and last Kitsu episode.
fn any_case() -> impl Strategy<Value = (Evidence, TrackerShow, Segment)> {
    (
        any_date(),
        any_date(),
        any_airdates(),
        any_date(),
        any_date(),
        1u32..=4,
        0u32..=2,
    )
        .prop_map(|(ks, ke, air, s, e, from, len)| {
            (
                evidence(ks, ke, air),
                tracker(s, e),
                segment(from, from + len),
            )
        })
}

/// A run with one bound a few days outside Kitsu's, across a month
/// boundary where `n` reaches past it: a start `n` days (0–5) before
/// Kitsu's start on one of its month's first three days, or an end `n`
/// days after Kitsu's end on one of its month's last three, the other
/// bound well inside — so that the two days' slack decides — with
/// Kitsu's air dates or none.
fn slack_case() -> impl Strategy<Value = (Evidence, TrackerShow, Segment)> {
    (
        (2020i32..=2020, 1u32..=12, 0u32..3),
        0usize..=5,
        any::<bool>(),
        prop_oneof![Just(BTreeMap::new()), any_airdates()],
        1u32..=4,
        0u32..=2,
    )
        .prop_map(|((y, m, k), n, at_start, air, from, len)| {
            let (before, after) = ("2019-06-15".to_string(), "2021-06-15".to_string());
            let (ks, ke, s, e) = if at_start {
                let kitsu = (y, m, 1 + k);
                let start = (0..n).fold(kitsu, |d, _| model::prev_day(d));
                (text(kitsu), after.clone(), text(start), after)
            } else {
                let kitsu = (y, m, model::month_length(y, m) - k);
                let end = (0..n).fold(kitsu, |d, _| model::next_day(d));
                (before.clone(), text(kitsu), before, text(end))
            };
            (
                evidence(Some(ks), Some(ke), air),
                tracker(Some(s), Some(e)),
                segment(from, from + len),
            )
        })
}

/// Whether `date` is a real date or month, by the model calendar.
fn real(date: Option<&str>) -> bool {
    date.is_some_and(model::is_real_date)
}

fn unplaced_numbers(ev: &Evidence, show: &TrackerShow, seg: &Segment) -> BTreeSet<u32> {
    unplaced_bounds(ev, show, seg)
        .unwrap_or_default()
        .iter()
        .map(|(n, _)| *n)
        .collect()
}

proptest! {
    /// The bounds are read only where all four dates are real dates or
    /// months; then at most the start and the end are unplaced, the
    /// start as the segment's first episode and the show's start, the
    /// end as its last episode and the show's end, in that order.
    #[test]
    fn unplaced_bounds_are_the_start_then_the_end_and_need_every_date(
        (ev, show, seg) in any_case()
    ) {
        let all_real = real(ev.kitsu.start.as_deref())
            && real(ev.kitsu.end.as_deref())
            && real(show.start.as_deref())
            && real(show.end.as_deref());
        let got = unplaced_bounds(&ev, &show, &seg);
        prop_assert_eq!(got.is_some(), all_real);
        if let Some(got) = got {
            let start = (seg.kitsu_from, show.start.as_deref().unwrap_or_default());
            let end = (seg.kitsu_to, show.end.as_deref().unwrap_or_default());
            prop_assert!(got.len() <= 2);
            let allowed = [vec![], vec![start], vec![end], vec![start, end]];
            prop_assert!(allowed.contains(&got), "{got:?}");
        }
    }

    /// Over real full dates: a start is placed when its month is no
    /// earlier than Kitsu's start's (or it is at most two days before
    /// Kitsu's start) and no later than Kitsu's end's month; an end when
    /// its month is no later than Kitsu's end's (or it is at most two
    /// days after Kitsu's end) and no earlier than Kitsu's start's.
    #[test]
    fn unplaced_bounds_match_their_model_over_real_dates(
        ks in real_date(), ke in real_date(), s in real_date(), e in real_date(),
        from in 1u32..=4, len in 0u32..=2
    ) {
        let ym = |(y, m, _): Ymd| (y, m);
        let start_inside = (ym(s) >= ym(ks) || ordinal(ks) - ordinal(s) <= 2) && ym(s) <= ym(ke);
        let end_inside = (ym(e) <= ym(ke) || ordinal(e) - ordinal(ke) <= 2) && ym(e) >= ym(ks);
        let (st, et) = (text(s), text(e));
        let seg = segment(from, from + len);
        let mut want = Vec::new();
        if !start_inside {
            want.push((seg.kitsu_from, st.as_str()));
        }
        if !end_inside {
            want.push((seg.kitsu_to, et.as_str()));
        }
        let ev = evidence(Some(text(ks)), Some(text(ke)), BTreeMap::new());
        let show = tracker(Some(st.clone()), Some(et.clone()));
        prop_assert_eq!(unplaced_bounds(&ev, &show, &seg), Some(want));
    }

    /// A run whose months lie within Kitsu's leaves nothing unplaced,
    /// and so lies inside Kitsu's run with no air dates at all.
    #[test]
    fn a_run_inside_kitsu_s_months_leaves_nothing_unplaced(
        mut dates in prop::collection::vec(real_date(), 4), swap in any::<bool>(),
        from in 1u32..=4, len in 0u32..=2
    ) {
        dates.sort_unstable();
        let (s, e) = if swap { (dates[2], dates[1]) } else { (dates[1], dates[2]) };
        let ev = evidence(Some(text(dates[0])), Some(text(dates[3])), BTreeMap::new());
        let show = tracker(Some(text(s)), Some(text(e)));
        let seg = segment(from, from + len);
        prop_assert_eq!(unplaced_bounds(&ev, &show, &seg), Some(vec![]));
        prop_assert!(inside_kitsu_run(&ev, &show, &seg));
    }

    /// A run is inside Kitsu's exactly as the documented rule decides
    /// it from the four dates and Kitsu's air dates, by the model
    /// calendar: every date real, each bound inside Kitsu's months
    /// (with two days' slack outwards) or placed by Kitsu's episode for
    /// it dated in the bound's month — over free dates, and over runs
    /// a few days outside Kitsu's across a month boundary.
    #[test]
    fn a_run_is_inside_exactly_when_every_unplaced_bound_is_dated_in(
        (ev, show, seg) in prop_oneof![any_case(), slack_case()]
    ) {
        let want = model::run_inside(&model::Run {
            kitsu_start: ev.kitsu.start.as_deref(),
            kitsu_end: ev.kitsu.end.as_deref(),
            start: show.start.as_deref(),
            end: show.end.as_deref(),
            from: seg.kitsu_from,
            to: seg.kitsu_to,
            airdates: &ev.kitsu_airdates,
        });
        prop_assert_eq!(inside_kitsu_run(&ev, &show, &seg), want);
    }

    /// More of Kitsu's air dates never move a run outside: an episode
    /// newly dated only adds a way to vouch for a bound.
    #[test]
    fn more_air_dates_never_move_a_run_outside(
        (ev, show, seg) in any_case(), more in any_airdates()
    ) {
        let mut wider = ev.clone();
        for (n, date) in more {
            wider.kitsu_airdates.entry(n).or_insert(date);
        }
        if inside_kitsu_run(&ev, &show, &seg) {
            prop_assert!(inside_kitsu_run(&wider, &show, &seg));
        }
    }

    /// Air dates for episodes no unplaced bound names never change
    /// whether the run is inside.
    #[test]
    fn air_dates_outside_the_unplaced_bounds_never_matter(
        (ev, show, seg) in any_case(), other in any_airdates()
    ) {
        let named = unplaced_numbers(&ev, &show, &seg);
        let mut swapped = ev.clone();
        swapped.kitsu_airdates = ev
            .kitsu_airdates
            .iter()
            .filter(|(n, _)| named.contains(n))
            .chain(other.iter().filter(|(n, _)| !named.contains(n)))
            .map(|(n, d)| (*n, d.clone()))
            .collect();
        prop_assert_eq!(
            inside_kitsu_run(&swapped, &show, &seg),
            inside_kitsu_run(&ev, &show, &seg)
        );
    }

    /// Kitsu's episode is dated in a bound's month exactly when both
    /// its air date and the bound are real dates or months and their
    /// `YYYY-MM` agree.
    #[test]
    fn an_episode_is_dated_in_a_bound_exactly_when_their_months_agree(
        air in any_airdates(), n in 0u32..=5, bound in any_date()
    ) {
        let ev = evidence(None, None, air.clone());
        let aired = model::month_of(air.get(&n).map(String::as_str));
        let want = aired.is_some() && aired == model::month_of(bound.as_deref());
        prop_assert_eq!(dated_in(&ev, n, bound.as_deref()), want);
    }
}
