//! Property coverage for the chain rule: a run is compared by the
//! month of a date only where the date is a real one; an exact chain
//! pairs and covers as the counts say; the rule admits exactly the
//! exact chains the dates place; and the air dates it asks for are the
//! ones it reads.
//!
//! Every strategy function returns a `BoxedStrategy`. Composed without
//! boxing, the nested combinator types grow into symbol names tens of
//! thousands of characters long, and the Windows test build's debug info
//! hits the linker's PDB limit (LNK4319). Box each new strategy too.

use super::super::dates::month;
use super::super::prop_tests::model;
use proptest::prelude::*;

/// Text as a date field could carry it: dates and months with every
/// field around its bounds, the end of February across leap and
/// century years, near misses, and anything at all.
fn date_text() -> BoxedStrategy<String> {
    prop_oneof![
        (0u32..10_000, 0u32..=13, 0u32..=32).prop_map(|(y, m, d)| format!("{y:04}-{m:02}-{d:02}")),
        (0u32..10_000, 0u32..=13).prop_map(|(y, m)| format!("{y:04}-{m:02}")),
        (
            prop::sample::select(vec![1900u32, 2000, 2023, 2024, 2100, 2400]),
            28u32..=30
        )
            .prop_map(|(y, d)| format!("{y:04}-02-{d:02}")),
        "[0-9-]{0,11}",
        ".{0,12}",
    ]
    .boxed()
}

proptest! {
    /// A date's month is its first seven characters exactly when the
    /// date reads as a real calendar date or month; any other text, and
    /// no date at all, has no month — and no text panics the read.
    #[test]
    fn a_month_is_read_exactly_from_a_real_date(text in date_text()) {
        let want = model::is_real_date(&text).then(|| &text[..7]);
        prop_assert_eq!(month(Some(&text)), want);
        prop_assert_eq!(month(None), None);
    }
}

use std::collections::{BTreeMap, BTreeSet};

use super::{chain, chain_dates_needed, exact_chain};
use crate::account::episode_map::{Evidence, KitsuShow, Shape, TrackerShow};

/// A date field as a source could send it, in a three-year window so
/// runs and months meet often: mostly real or a month alone, so that
/// whole chains are dated, sometimes impossible, garbled or absent.
fn any_date() -> BoxedStrategy<Option<String>> {
    prop_oneof![
        1 => Just(None),
        12 => known_date().prop_map(Some),
        1 => Just(Some("2020-02-30".to_string())),
        1 => "[0-9-]{0,11}".prop_map(Some),
    ]
    .boxed()
}

/// A date the dates rule reads: a real full date, often a few days
/// from a month boundary where the slack decides, or a month.
fn known_date() -> BoxedStrategy<String> {
    prop_oneof![
        model::real_date_in(2019..=2021).prop_map(model::text),
        model::edge_date_in(2019..=2021).prop_map(model::text),
        (2019i32..=2021, 1u32..=12).prop_map(|(y, m)| format!("{y:04}-{m:02}")),
    ]
    .boxed()
}

/// A hop's count: unknown, zero, small, or near the top of the range.
fn hop_count() -> BoxedStrategy<Option<u32>> {
    prop_oneof![
        1 => Just(None),
        1 => Just(Some(0)),
        6 => (1u32..=6).prop_map(Some),
        1 => (u32::MAX - 6..=u32::MAX).prop_map(Some),
    ]
    .boxed()
}

fn tracker(
    id: u32,
    episodes: Option<u32>,
    finished: bool,
    start: Option<String>,
    end: Option<String>,
) -> TrackerShow {
    TrackerShow {
        id,
        id_mal: None,
        episodes,
        finished,
        start,
        end,
    }
}

/// Evidence for a chain with Kitsu's count and the entry's total: the
/// count is often the total plus a prefix of the hops, so exact chains
/// come up, and is otherwise any count at all. Kitsu's air dates are
/// mostly each segment bound's episode dated with that bound's own
/// date, so that they place bounds often enough to matter, over a few
/// episodes dated with any text.
fn chain_case() -> BoxedStrategy<(Evidence, u32, u32)> {
    let claim = prop_oneof![5 => Just(false), 1 => Just(true)];
    let hop = (hop_count(), any::<bool>(), any_date(), any_date(), claim);
    let own_dated = prop::collection::vec(prop_oneof![1 => Just(false), 3 => Just(true)], 10);
    let noise =
        prop::collection::btree_map(0u32..=24, any_date().prop_filter_map("a date", |d| d), 0..3);
    (
        prop_oneof![3 => 1u32..=6, 1 => any::<u32>()],
        prop::collection::vec(hop, 0..=4),
        (any::<bool>(), any_date(), any_date()),
        (any_date(), any_date()),
        prop_oneof![6 => (0usize..=4).prop_map(Ok), 1 => any::<u32>().prop_map(Err)],
        (own_dated, noise),
        any::<bool>(),
    )
        .prop_map(
            |(total, hops, entry, (ks, ke), count, (own_dated, noise), claim_entry)| {
                let (finished, s, e) = entry;
                let mut ev = Evidence {
                    kitsu: KitsuShow {
                        id: "k".into(),
                        episodes: None,
                        start: ks,
                        end: ke,
                    },
                    entry: tracker(1, Some(total), finished, s, e),
                    ..Default::default()
                };
                for (i, (n, f, s, e, claimed)) in hops.into_iter().enumerate() {
                    let id = 2 + u32::try_from(i).unwrap_or(0);
                    ev.chain.push(tracker(id, n, f, s, e));
                    if claimed {
                        ev.claimed.push(id);
                    }
                }
                if claim_entry {
                    ev.claimed.push(1);
                }
                // Every segment's first and last episode, were each hop taken.
                let mut bounds = vec![1, total];
                let mut covered = total;
                for hop in &ev.chain {
                    let n = hop.episodes.unwrap_or(0);
                    bounds.extend([covered.wrapping_add(1), covered.wrapping_add(n)]);
                    covered = covered.wrapping_add(n);
                }
                let kitsu_count = match count {
                    Ok(k) => bounds.get(2 * k + 1).copied().unwrap_or(covered),
                    Err(any) => any,
                };
                ev.kitsu.episodes = Some(kitsu_count);
                // A bound's own date: the segment's start for its first
                // episode, its end for its last.
                let dates: Vec<Option<String>> = std::iter::once(&ev.entry)
                    .chain(&ev.chain)
                    .flat_map(|s| [s.start.clone(), s.end.clone()])
                    .collect();
                ev.kitsu_airdates = noise;
                for ((number, date), dated) in bounds.iter().zip(dates).zip(own_dated) {
                    if let (Some(date), true) = (date, dated) {
                        ev.kitsu_airdates.insert(*number, date);
                    }
                }
                (ev, kitsu_count, total)
            },
        )
        .boxed()
}

/// An exact, unclaimed chain of two to four entries whose every date
/// is a real date or month, with no air dates yet.
fn dated_exact_chain() -> BoxedStrategy<Evidence> {
    (
        prop::collection::vec((1u32..=4, any::<bool>(), known_date(), known_date()), 2..=4),
        known_date(),
        known_date(),
    )
        .prop_map(|(mut shows, ks, ke)| {
            let count: u32 = shows.iter().map(|(n, ..)| n).sum();
            // In the order they aired: a hop that aired before the one
            // it follows is no chain.
            shows.sort_by(|a, b| a.2.cmp(&b.2));
            let mut shows: Vec<TrackerShow> = shows
                .into_iter()
                .enumerate()
                .map(|(i, (n, f, s, e))| {
                    tracker(
                        1 + u32::try_from(i).unwrap_or(0),
                        Some(n),
                        f,
                        Some(s),
                        Some(e),
                    )
                })
                .collect();
            let entry = shows.remove(0);
            Evidence {
                kitsu: KitsuShow {
                    id: "k".into(),
                    episodes: Some(count),
                    start: Some(ks),
                    end: Some(ke),
                },
                entry,
                chain: shows,
                ..Default::default()
            }
        })
        .boxed()
}

proptest! {
    // Chains that air dates place are a small share of the cases; more
    // cases reach them.
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// An exact chain pairs each segment with the entry it files, in
    /// chain order from the entry itself, over at least one hop; each
    /// segment is the whole of its entry.
    #[test]
    fn an_exact_chain_pairs_each_segment_with_its_entry((ev, count, total) in chain_case()) {
        let Some((segments, shows)) = exact_chain(&ev, count, total) else { return Ok(()) };
        prop_assert_eq!(segments.len(), shows.len());
        prop_assert!(segments.len() >= 2);
        prop_assert_eq!(shows[0], &ev.entry);
        prop_assert_eq!(segments[0].entry_total, total);
        for (i, (segment, show)) in segments.iter().zip(&shows).enumerate() {
            if i > 0 {
                prop_assert_eq!(*show, &ev.chain[i - 1]);
                prop_assert_eq!(Some(segment.entry_total), show.episodes);
            }
            prop_assert_eq!(segment.entry, show.id);
            prop_assert_eq!(segment.entry_finished, show.finished);
            prop_assert_eq!(segment.lead, 0);
            prop_assert!(segment.extras_after.is_empty());
        }
    }

    /// An exact chain from an entry with episodes covers Kitsu's
    /// `1..=count` contiguously, each segment its entry's count long.
    #[test]
    fn an_exact_chain_covers_kitsu_s_episodes_contiguously((ev, count, total) in chain_case()) {
        let Some((segments, _)) = exact_chain(&ev, count, total) else { return Ok(()) };
        prop_assume!(total >= 1);
        prop_assert_eq!(segments[0].kitsu_from, 1);
        prop_assert_eq!(segments.last().map(|s| s.kitsu_to), Some(count));
        for pair in segments.windows(2) {
            prop_assert_eq!(u64::from(pair[0].kitsu_to) + 1, u64::from(pair[1].kitsu_from));
        }
        for s in &segments {
            prop_assert_eq!(
                i64::from(s.kitsu_to) - i64::from(s.kitsu_from) + 1,
                i64::from(s.entry_total)
            );
        }
    }

    /// No segment past the first is an entry some Kitsu entry claims;
    /// the first may be.
    #[test]
    fn an_exact_chain_claims_nothing_past_its_first((ev, count, total) in chain_case()) {
        let Some((segments, _)) = exact_chain(&ev, count, total) else { return Ok(()) };
        prop_assert!(segments[1..].iter().all(|s| !ev.claimed.contains(&s.entry)));
    }

    /// Any counts at all, Kitsu's and the entry's, are answered without
    /// a panic by the exact chain, the chain rule and its dates.
    #[test]
    fn the_chain_rule_answers_any_counts(
        (ev, _, _) in chain_case(), count in any::<u32>(), total in any::<u32>()
    ) {
        let _ = exact_chain(&ev, count, total);
        let _ = chain(&ev, count, total);
        let _ = chain_dates_needed(&ev, count, total);
    }

    /// The chain rule as `docs/title-resolution.md` states it: the
    /// counts, summed independently, make an exact unclaimed chain, and
    /// every entry's run lies inside Kitsu's by month with two days'
    /// slack, or is placed by Kitsu's own episode list — decided from
    /// the dates by the model calendar, not by the dates rule.
    #[test]
    fn the_chain_rule_is_the_exact_chain_its_dates_place((ev, count, total) in chain_case()) {
        let want = model::exact_chain_model(&ev, count, total)
            .filter(|links| {
                links.iter().all(|link| {
                    model::run_inside(&model::Run {
                        kitsu_start: ev.kitsu.start.as_deref(),
                        kitsu_end: ev.kitsu.end.as_deref(),
                        start: link.show.start.as_deref(),
                        end: link.show.end.as_deref(),
                        from: link.from,
                        to: link.to,
                        airdates: &ev.kitsu_airdates,
                    })
                })
            })
            .map(|links| links.iter().map(model::segment_of).collect::<Vec<_>>());
        prop_assert_eq!(chain(&ev, count, total), want);
    }

    /// The dates needed are exactly the documented rule's: where the
    /// counts build an exact, unclaimed chain and every link's four
    /// dates are real, each bound the dates alone leave outside Kitsu's
    /// run, as its segment's first or last Kitsu episode; none otherwise
    /// — decided by the model calendar and chain, not by the dates rule.
    #[test]
    fn the_dates_needed_are_an_exact_chain_s_unplaced_bounds((ev, count, total) in chain_case()) {
        prop_assert_eq!(
            chain_dates_needed(&ev, count, total),
            model::dates_needed_model(&ev, count, total)
        );
    }

    /// Kitsu's air dates for the episodes the gatherer is told it needs
    /// are all the map reads: the map from them alone is the map from
    /// every air date.
    #[test]
    fn the_dates_needed_suffice_for_the_map((ev, _, _) in chain_case()) {
        let needed = super::super::chain_dates_needed(&ev);
        let mut restricted = ev.clone();
        restricted.kitsu_airdates.retain(|n, _| needed.contains(n));
        prop_assert_eq!(super::super::map_show(&restricted), super::super::map_show(&ev));
    }

    /// An exact, unclaimed chain with every date known is admitted once
    /// each needed episode is dated in its bound's month, the months
    /// taken from the model's unplaced bounds.
    #[test]
    fn an_exact_chain_dated_where_needed_is_admitted(mut ev in dated_exact_chain()) {
        let count = ev.kitsu.episodes.unwrap_or(0);
        let total = ev.entry.episodes.unwrap_or(0);
        let Some(links) = model::exact_chain_model(&ev, count, total) else {
            prop_assert!(false, "the chain is exact and unclaimed");
            unreachable!()
        };
        let mut months: BTreeMap<u32, String> = BTreeMap::new();
        for link in &links {
            let unplaced = model::unplaced(&model::run_of(&ev, link));
            prop_assert!(unplaced.is_some(), "every date is known");
            for (n, (y, m)) in unplaced.unwrap_or_default() {
                let month = format!("{y:04}-{m:02}-01");
                // A one-episode segment whose start and end are both
                // unplaced, in different months, no single date places.
                if months.get(&n).is_some_and(|d| *d != month) {
                    return Ok(());
                }
                months.insert(n, month);
            }
        }
        let segments: Vec<_> = links.iter().map(model::segment_of).collect();
        prop_assert_eq!(
            super::super::chain_dates_needed(&ev),
            months.keys().copied().collect::<BTreeSet<u32>>()
        );
        ev.kitsu_airdates = months;
        let map = super::super::map_show(&ev);
        prop_assert_eq!(map.shape, Shape::Chain);
        prop_assert_eq!(map.segments, segments);
    }
}
