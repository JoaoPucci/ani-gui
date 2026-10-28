//! Property coverage for the episode map: whatever evidence a rule
//! fires on, the segments cover Kitsu's episodes once each and in
//! order, every episode lands inside its entry, nothing moves backwards,
//! and a write completes an entry only at the entry's own last episode.
//!
//! Every strategy function returns a `BoxedStrategy`. Composed without
//! boxing, the nested combinator types grow into symbol names tens of
//! thousands of characters long, and the Windows test build's debug info
//! hits the linker's PDB limit (LNK4319). Box each new strategy too.

use super::extras_rule::first_part_extras;
use super::parts_rule::split_parts;
use super::*;
use crate::account::episode_map::{EpisodeMap, Evidence, KitsuShow, Shape, Target, TrackerShow};
use proptest::prelude::*;

/// The calendar and chain models the date and chain properties across
/// the rules' test modules compare against.
#[path = "episode_map_model_test.rs"]
pub(super) mod model;

fn kitsu(id: &str, episodes: u32, start: &str, end: &str) -> KitsuShow {
    KitsuShow {
        id: id.into(),
        episodes: Some(episodes),
        start: Some(start.into()),
        end: Some(end.into()),
    }
}

fn show(id: u32, episodes: u32, finished: bool) -> TrackerShow {
    TrackerShow {
        id,
        id_mal: None,
        episodes: Some(episodes),
        finished,
        start: Some("2001-03-05".into()),
        end: Some("2009-10-20".into()),
    }
}

/// A chain of two to five finished-or-not segments, every one inside
/// Kitsu's 2000–2010 run, summing to Kitsu's count.
fn chain_evidence() -> BoxedStrategy<Evidence> {
    prop::collection::vec((1u32..=60, any::<bool>()), 2..=5)
        .prop_map(|segments| {
            let count: u32 = segments.iter().map(|(n, _)| n).sum();
            let shows: Vec<TrackerShow> = segments
                .iter()
                .enumerate()
                .map(|(i, (n, finished))| show(100 + u32::try_from(i).unwrap_or(0), *n, *finished))
                .collect();
            Evidence {
                kitsu: kitsu("k", count, "2000-01-01", "2010-12-31"),
                entry: shows[0].clone(),
                chain: shows[1..].to_vec(),
                ..Default::default()
            }
        })
        .boxed()
}

/// A list of Kitsu's episodes plus extras after some of them, each
/// extra once or twice (as AniList lists a subbed and a dubbed track).
fn extras_evidence() -> BoxedStrategy<(Evidence, Vec<u32>)> {
    (1u32..=40, any::<bool>())
        .prop_flat_map(|(count, finished)| {
            prop::collection::btree_set(0..=count, 0..=4).prop_map(move |after| {
                let after: Vec<u32> = after.into_iter().collect();
                let mut listed: Vec<String> = (1..=count).map(|n| n.to_string()).collect();
                for e in &after {
                    let text = if *e == 0 {
                        "0".to_string()
                    } else {
                        format!("{e}.5")
                    };
                    listed.push(text.clone());
                    listed.push(text);
                }
                let surplus = u32::try_from(after.len()).unwrap_or(0);
                let ev = Evidence {
                    kitsu: kitsu("k", count, "2020-01-01", "2020-12-31"),
                    entry: show(7, count + surplus, finished),
                    listed: Some(listed),
                    ..Default::default()
                };
                (ev, after)
            })
        })
        .boxed()
}

/// Two to four Kitsu parts under one entry, one of them the show mapped.
fn parts_evidence() -> BoxedStrategy<(Evidence, usize)> {
    prop::collection::vec(1u32..=60, 2..=4)
        .prop_flat_map(|counts| {
            let n = counts.len();
            (Just(counts), 0..n, any::<bool>())
        })
        .prop_map(|(counts, ours, finished)| {
            let total: u32 = counts.iter().sum();
            let parts: Vec<KitsuShow> = counts
                .iter()
                .enumerate()
                .map(|(i, n)| kitsu(&format!("p{i}"), *n, &format!("20{i:02}-01-01"), ""))
                .collect();
            let ev = Evidence {
                kitsu: parts[ours].clone(),
                entry: show(9, total, finished),
                parts,
                ..Default::default()
            };
            (ev, ours)
        })
        .boxed()
}

/// Every mapped episode's write, in Kitsu order, with the segment it
/// came from; asserts the invariants every shape shares.
fn check_mapped(map: &EpisodeMap, count: u32) -> Result<(), TestCaseError> {
    prop_assert!(!map.segments.is_empty());
    prop_assert_eq!(map.segments[0].kitsu_from, 1);
    prop_assert_eq!(map.segments.last().map(|s| s.kitsu_to), Some(count));
    for pair in map.segments.windows(2) {
        prop_assert_eq!(pair[0].kitsu_to + 1, pair[1].kitsu_from);
    }
    let mut last: Option<(u32, u32)> = None;
    for k in 1..=count {
        let Target::Mapped(write) = map.target(k) else {
            prop_assert!(false, "episode {k} unmapped");
            unreachable!()
        };
        let seg = map
            .segments
            .iter()
            .find(|s| s.kitsu_from <= k && k <= s.kitsu_to)
            .expect("a segment covers k");
        prop_assert_eq!(write.entry, seg.entry);
        prop_assert!(write.progress >= 1 && write.progress <= seg.entry_total);
        if let Some((entry, progress)) = last {
            if entry == write.entry {
                prop_assert!(write.progress > progress, "progress moved backwards at {k}");
            }
        }
        prop_assert_eq!(
            write.complete,
            k == seg.kitsu_to && seg.entry_finished && write.progress == seg.entry_total
        );
        prop_assert_eq!(
            write.earlier.len(),
            map.segments
                .iter()
                .position(|s| s.entry == seg.entry && s.kitsu_from == seg.kitsu_from)
                .unwrap_or(0)
        );
        last = Some((write.entry, write.progress));
    }
    prop_assert_eq!(map.target(count + 1), Target::Identity);
    prop_assert_eq!(map.target(0), Target::Identity);
    Ok(())
}

proptest! {
    #[test]
    fn a_chain_covers_kitsu_s_episodes_once_each_in_order(ev in chain_evidence()) {
        let map = map_show(&ev);
        prop_assert_eq!(map.shape, Shape::Chain);
        let count = ev.kitsu.episodes.unwrap_or(0);
        check_mapped(&map, count)?;
        // Each segment is the whole of its entry.
        for s in &map.segments {
            prop_assert_eq!(s.kitsu_to - s.kitsu_from + 1, s.entry_total);
            prop_assert_eq!(s.lead, 0);
        }
    }

    #[test]
    fn extras_shift_each_episode_by_the_extras_before_it((ev, after) in extras_evidence()) {
        let map = map_show(&ev);
        let count = ev.kitsu.episodes.unwrap_or(0);
        let total = ev.entry.episodes.unwrap_or(0);
        if after.is_empty() {
            prop_assert_eq!(map.shape, Shape::Identity(Identity::CountsAgree));
            return Ok(());
        }
        prop_assert_eq!(map.shape, Shape::Extras);
        check_mapped(&map, count)?;
        for k in 1..count {
            let before = u32::try_from(after.iter().filter(|e| **e < k).count()).unwrap_or(0);
            let Target::Mapped(write) = map.target(k) else { unreachable!() };
            prop_assert_eq!(write.progress, k + before);
        }
        let Target::Mapped(finale) = map.target(count) else { unreachable!() };
        prop_assert_eq!(finale.progress, total);
        prop_assert_eq!(finale.complete, ev.entry.finished);
    }

    #[test]
    fn a_part_s_episodes_are_the_entry_s_after_the_parts_before_it((ev, ours) in parts_evidence()) {
        let map = map_show(&ev);
        prop_assert_eq!(map.shape, Shape::SplitParts);
        let count = ev.kitsu.episodes.unwrap_or(0);
        check_mapped(&map, count)?;
        let lead: u32 = ev.parts[..ours].iter().filter_map(|p| p.episodes).sum();
        prop_assert_eq!(map.segments[0].lead, lead);
        let Target::Mapped(first) = map.target(1) else { unreachable!() };
        prop_assert_eq!(first.progress, lead + 1);
        let Target::Mapped(last) = map.target(count) else { unreachable!() };
        let is_last_part = ours + 1 == ev.parts.len();
        prop_assert_eq!(last.complete, is_last_part && ev.entry.finished);
    }

    #[test]
    fn the_same_parts_mapped_to_the_entry_read_the_same_in_date_order((ev, _ours) in parts_evidence()) {
        // The shared-entry rule over the same parts, given as Kitsu's
        // mapped entries in any order, agrees with the split-parts rule.
        let mut shared = ev.clone();
        shared.sharing = ev.parts.iter().rev().cloned().collect();
        shared.parts.clear();
        let as_shared = map_show(&shared);
        let as_parts = map_show(&ev);
        prop_assert_eq!(as_shared.shape, Shape::SharedEntry);
        prop_assert_eq!(as_shared.segments, as_parts.segments);
    }

    #[test]
    fn agreeing_counts_are_identity_whatever_else_the_evidence_says(ev in chain_evidence(), n in 1u32..=300) {
        let mut ev = ev;
        ev.kitsu.episodes = Some(n);
        ev.entry.episodes = Some(n);
        prop_assert_eq!(map_show(&ev), EpisodeMap::identity(Identity::CountsAgree));
    }

    #[test]
    fn a_listed_number_reads_back_as_it_was_written(whole in 0u32..10_000, half in any::<bool>()) {
        let text = if half { format!("{whole}.5") } else { whole.to_string() };
        prop_assert_eq!(listed_number(&text), Some((whole, half.then_some("5"))));
    }

    #[test]
    fn a_map_round_trips_through_its_cache_row(ev in chain_evidence()) {
        let map = map_show(&ev);
        let row = serde_json::to_string(&map).expect("serialise");
        let back: EpisodeMap = serde_json::from_str(&row).expect("parse");
        prop_assert_eq!(back, map);
    }
}

#[test]
fn an_identity_map_round_trips_with_its_reason() {
    let map = EpisodeMap::identity(Identity::Unexplained);
    let row = serde_json::to_string(&map).expect("serialise");
    assert_eq!(
        row,
        r#"{"shape":"identity","why":"unexplained","segments":[]}"#
    );
    let back: EpisodeMap = serde_json::from_str(&row).expect("parse");
    assert_eq!(back, map);
}

#[test]
fn a_cache_row_this_build_cannot_read_is_no_map() {
    assert!(serde_json::from_str::<EpisodeMap>(r#"{"shape":"orbit"}"#).is_err());
    assert!(serde_json::from_str::<EpisodeMap>("[]").is_err());
}

/// An episode count as any source could send it: small, zero, unknown,
/// or near the top of the range.
fn any_count() -> BoxedStrategy<Option<u32>> {
    prop_oneof![
        Just(None),
        (0u32..=40).prop_map(Some),
        (u32::MAX - 40..=u32::MAX).prop_map(Some),
        any::<u32>().prop_map(Some),
    ]
    .boxed()
}

/// A date as any source could send it: real, partial, unreal or absent.
fn any_date() -> BoxedStrategy<Option<String>> {
    prop_oneof![
        Just(None),
        Just(Some("2001-03-05".to_string())),
        Just(Some("2009-10".to_string())),
        Just(Some("2005".to_string())),
        Just(Some("2001-02-31".to_string())),
        "[0-9-]{0,12}".prop_map(Some),
    ]
    .boxed()
}

fn any_tracker_show(id: u32) -> BoxedStrategy<TrackerShow> {
    (any_count(), any::<bool>(), any_date(), any_date())
        .prop_map(move |(episodes, finished, start, end)| TrackerShow {
            id,
            id_mal: None,
            episodes,
            finished,
            start,
            end,
        })
        .boxed()
}

fn any_kitsu_show(id: &'static str) -> BoxedStrategy<KitsuShow> {
    (any_count(), any_date(), any_date())
        .prop_map(move |(episodes, start, end)| KitsuShow {
            id: id.into(),
            episodes,
            start,
            end,
        })
        .boxed()
}

/// Kitsu's air dates as its episode list could give them: a few
/// episode numbers, small or anywhere in the range, each dated with any
/// text a date field could carry.
fn any_airdates() -> BoxedStrategy<std::collections::BTreeMap<u32, String>> {
    prop::collection::btree_map(
        prop_oneof![3 => 0u32..=12, 1 => any::<u32>()],
        any_date().prop_filter_map("a date", |d| d),
        0..4,
    )
    .boxed()
}

/// Evidence as the gatherer could hand it over, every field free.
fn any_evidence() -> BoxedStrategy<Evidence> {
    (
        any_kitsu_show("k"),
        any_tracker_show(1),
        prop::collection::vec(any_tracker_show(2), 0..4),
        any::<bool>(),
        proptest::option::of(prop::collection::vec("[0-9.]{0,5}", 0..6)),
        prop::collection::vec(any_kitsu_show("s"), 0..4),
        prop::collection::vec(any_kitsu_show("p"), 0..4),
        any_airdates(),
    )
        .prop_map(
            |(kitsu, entry, mut chain, claim, listed, sharing, parts, kitsu_airdates)| {
                for (i, hop) in chain.iter_mut().enumerate() {
                    hop.id = 2 + u32::try_from(i).unwrap_or(0);
                }
                let claimed = if claim {
                    chain.iter().map(|s| s.id).take(1).collect()
                } else {
                    Vec::new()
                };
                Evidence {
                    kitsu,
                    entry,
                    chain,
                    claimed,
                    listed,
                    sharing,
                    kitsu_airdates,
                    parts,
                }
            },
        )
        .boxed()
}

/// Free evidence, often with Kitsu's count moved to the entry's plus a
/// prefix of its hops', so that exact chains come up, and often with
/// every run dated from a few real dates and months either side of a
/// month boundary, so that those chains need dates.
fn chain_prone_evidence() -> BoxedStrategy<Evidence> {
    let real = prop::sample::select(vec![
        "2001-03-05",
        "2001-03",
        "2001-02-27",
        "2005-06-15",
        "2009-10",
        "2009-10-31",
        "2009-11-01",
    ]);
    (
        any_evidence(),
        prop::option::weighted(0.7, 1usize..=4),
        prop::option::weighted(0.7, prop::collection::vec(real, 10)),
    )
        .prop_map(|(mut ev, hops, dates)| {
            if let Some(hops) = hops {
                let sum: u64 = std::iter::once(&ev.entry)
                    .chain(ev.chain.iter().take(hops))
                    .map(|s| u64::from(s.episodes.unwrap_or(0)))
                    .sum();
                if let Ok(count) = u32::try_from(sum) {
                    ev.kitsu.episodes = Some(count);
                }
            }
            if let Some(dates) = dates {
                let mut dates = dates.into_iter().map(|d| Some(d.to_string()));
                ev.kitsu.start = dates.next().flatten();
                ev.kitsu.end = dates.next().flatten();
                for show in std::iter::once(&mut ev.entry).chain(ev.chain.iter_mut()) {
                    show.start = dates.next().flatten();
                    show.end = dates.next().flatten();
                }
            }
            ev
        })
        .boxed()
}

/// Evidence one of the rules fires on, then one field of it nudged: a
/// count off by one, a finished flag flipped, a hop unknown or claimed,
/// a date unreal, the list or a part one short. Some nudges leave the
/// rule firing, some break it, some wake a rival.
fn near_rule_evidence() -> BoxedStrategy<Evidence> {
    let base = prop_oneof![
        chain_evidence(),
        extras_evidence().prop_map(|(ev, _)| ev),
        parts_evidence().prop_map(|(ev, _)| ev),
        filing_evidence().prop_map(|(ev, _)| ev),
    ];
    (base, 0u8..10)
        .prop_map(|(mut ev, nudge)| {
            // The show's count moves with it wherever the evidence names it,
            // so the nudged evidence still agrees with itself. Parts that
            // disagree with the show are their own property
            // (a_part_whose_count_is_not_the_show_s_own_files_nothing_past_the_show).
            let recount = |ev: &mut Evidence, f: fn(u32) -> u32| {
                let ours = ev.kitsu.id.clone();
                ev.kitsu.episodes = ev.kitsu.episodes.map(f);
                for k in ev.parts.iter_mut().chain(ev.sharing.iter_mut()) {
                    if k.id == ours {
                        k.episodes = k.episodes.map(f);
                    }
                }
            };
            match nudge {
                1 => recount(&mut ev, |n| n.saturating_add(1)),
                2 => recount(&mut ev, |n| n.saturating_sub(1)),
                3 => ev.entry.finished = !ev.entry.finished,
                4 => {
                    if let Some(hop) = ev.chain.first_mut() {
                        hop.episodes = None;
                    }
                }
                5 => ev.claimed = ev.chain.iter().map(|s| s.id).take(1).collect(),
                6 => ev.kitsu.start = Some("2001-02-31".into()),
                7 => {
                    if let Some(listed) = ev.listed.as_mut() {
                        listed.pop();
                    }
                }
                8 => {
                    ev.parts.pop();
                }
                9 => {
                    ev.sharing.pop();
                }
                _ => {}
            }
            ev
        })
        .boxed()
}

proptest! {
    /// Whatever evidence arrives, the rules answer: they never panic,
    /// whatever counts or dates the services sent. A map they answer
    /// with a small count holds the invariants every shape shares —
    /// checked on evidence near a rule as well as on free evidence,
    /// since free evidence almost never makes a rule fire.
    #[test]
    fn any_evidence_maps_without_panicking(
        ev in prop_oneof![any_evidence(), near_rule_evidence()]
    ) {
        let map = map_show(&ev);
        if let (Shape::Identity(_), true) = (map.shape, map.segments.is_empty()) {
            return Ok(());
        }
        if let Some(count) = ev.kitsu.episodes.filter(|n| *n <= 500) {
            check_mapped(&map, count)?;
        }
    }

    /// The episodes the gatherer is told it needs are Kitsu's own: each
    /// one lies within `1..=` Kitsu's count, and none is asked for while
    /// that count is unknown or zero.
    #[test]
    fn the_dates_needed_are_kitsu_s_own_episodes(ev in chain_prone_evidence()) {
        let needed = chain_dates_needed(&ev);
        let count = ev.kitsu.episodes.unwrap_or(0);
        prop_assert!(needed.iter().all(|n| (1..=count).contains(n)), "{needed:?} past {count}");
    }

    /// The episodes needed bound the exact chain the counts alone build:
    /// each is the first or the last Kitsu episode of one of its
    /// segments, and there are none where the counts build no exact
    /// chain — the entry's count unknown, zero or not below Kitsu's, a
    /// hop's count unknown before the sum reaches Kitsu's, the sum
    /// passing it, or a hop a Kitsu entry claims.
    #[test]
    fn the_dates_needed_bound_the_exact_chain_the_counts_build(ev in chain_prone_evidence()) {
        let needed = chain_dates_needed(&ev);
        let links = match (ev.kitsu.episodes, ev.entry.episodes) {
            (Some(count), Some(total)) if total > 0 => model::exact_chain_model(&ev, count, total),
            _ => None,
        };
        let bounds: std::collections::BTreeSet<u32> = links
            .iter()
            .flatten()
            .flat_map(|link| [link.from, link.to])
            .collect();
        prop_assert!(needed.is_subset(&bounds), "{needed:?} not in {bounds:?}");
    }

    /// No dates are needed where no prefix of the entry and its chain —
    /// the entry's count plus any number of hops', unknown counted as
    /// none, summed in `u64` — is Kitsu's count.
    #[test]
    fn no_dates_are_needed_where_no_prefix_sums_to_kitsu_s_count(ev in chain_prone_evidence()) {
        let count = ev.kitsu.episodes.map(u64::from);
        let mut sum = u64::from(ev.entry.episodes.unwrap_or(0));
        let mut reached = false;
        for hop in &ev.chain {
            sum += u64::from(hop.episodes.unwrap_or(0));
            reached |= count == Some(sum);
        }
        if !reached {
            prop_assert!(chain_dates_needed(&ev).is_empty());
        }
    }

    /// The episodes needed are exactly the ones the documented chain
    /// rule reads Kitsu's episode list for: where the counts build an
    /// exact, unclaimed chain from an entry with episodes and every
    /// link's four dates are real, each bound the dates alone leave
    /// outside Kitsu's run, as its segment's first or last Kitsu episode
    /// ([`model::dates_needed_model`]); none otherwise.
    #[test]
    fn the_dates_needed_are_the_unplaced_bounds_of_a_dated_exact_chain(
        ev in chain_prone_evidence()
    ) {
        let want = match (ev.kitsu.episodes, ev.entry.episodes) {
            (Some(count), Some(total)) if total > 0 => model::dates_needed_model(&ev, count, total),
            _ => std::collections::BTreeSet::new(),
        };
        prop_assert_eq!(chain_dates_needed(&ev), want);
    }

    /// Whatever the evidence, the air dates the gatherer is told it
    /// needs are all the map reads of them — over free evidence, and
    /// over evidence prone to exact chains whose dates need placing,
    /// where the dates needed are not empty.
    #[test]
    fn any_evidence_maps_the_same_from_the_dates_needed_alone(
        ev in prop_oneof![any_evidence(), chain_prone_evidence()]
    ) {
        let needed = chain_dates_needed(&ev);
        let mut restricted = ev.clone();
        restricted.kitsu_airdates.retain(|n, _| needed.contains(n));
        prop_assert_eq!(map_show(&restricted), map_show(&ev));
    }
}

/// A count as the part-filing rules see one: mostly small so parts sum
/// to a total often, sometimes unknown or zero, sometimes near the top
/// of the range where a sum overflows.
fn part_count() -> BoxedStrategy<Option<u32>> {
    prop_oneof![
        1 => Just(None),
        1 => Just(Some(0)),
        6 => (1u32..=6).prop_map(Some),
        1 => (u32::MAX - 6..=u32::MAX).prop_map(Some),
    ]
    .boxed()
}

/// An entry total to set against those parts.
fn part_total() -> BoxedStrategy<u32> {
    prop_oneof![6 => 1u32..=20, 1 => u32::MAX - 6..=u32::MAX].boxed()
}

/// A Kitsu id from a pool small enough that parts, the mapped set and
/// the show itself name the same entries often.
fn pooled_id() -> BoxedStrategy<String> {
    (0u8..5).prop_map(|i| format!("p{i}")).boxed()
}

fn pooled_kitsu() -> BoxedStrategy<KitsuShow> {
    (pooled_id(), part_count(), any::<bool>())
        .prop_map(|(id, episodes, dated)| KitsuShow {
            id,
            episodes,
            start: dated.then(|| "2020-01-01".to_string()),
            end: None,
        })
        .boxed()
}

/// Evidence where Kitsu's parts and the entries mapped to the tracker's
/// entry overlap: free evidence of that kind, and evidence built so a
/// rule fires or nearly does.
fn filing_evidence() -> BoxedStrategy<(Evidence, u32)> {
    prop_oneof![free_filing_evidence(), firing_filing_evidence()].boxed()
}

/// Evidence where Kitsu's parts and the entries mapped to the tracker's
/// entry overlap: the show one of the parts or not, the mapped set a
/// mix of the parts and others, the entry's list of numbers present or
/// not. Every field is drawn on its own, so a rule seldom fires.
fn free_filing_evidence() -> BoxedStrategy<(Evidence, u32)> {
    (
        pooled_kitsu(),
        prop::collection::vec(pooled_kitsu(), 0..5),
        prop::collection::vec(pooled_kitsu(), 0..4),
        proptest::option::of(prop::collection::vec("0|[1-9]|[1-9]\\.5|x", 0..8)),
        part_total(),
        any::<bool>(),
    )
        .prop_map(|(kitsu, parts, sharing, listed, total, finished)| {
            let ev = Evidence {
                kitsu,
                entry: TrackerShow {
                    id: 9,
                    id_mal: None,
                    episodes: Some(total),
                    finished,
                    start: None,
                    end: None,
                },
                listed,
                sharing,
                parts,
                ..Default::default()
            };
            (ev, total)
        })
        .boxed()
}

/// How the entries mapped to the tracker's entry stand to Kitsu's chain
/// of parts in [`firing_filing_evidence`].
#[derive(Debug, Clone, Copy)]
enum Mapped {
    /// The chain's parts exactly: the shared-entry rule and the chain
    /// agree.
    Same,
    /// The chain's parts with one besides the show renamed: the set
    /// still sums to the total, and the chain files the entry otherwise.
    Outsider,
    /// The chain's parts without one besides the show: the set falls
    /// short of the total.
    Missing,
    /// The chain's parts and the first part past the total: the chain
    /// is contradicted.
    Late,
    /// The chain's parts, one of them without a start date.
    Undated,
    /// None mapped.
    Empty,
}

/// `YYYY-MM-DD` for the `day`th day of 2020 counting 28 to a month:
/// distinct days give distinct, real, full dates.
fn day_of_2020(day: u32) -> String {
    format!("2020-{:02}-{:02}", day / 28 + 1, day % 28 + 1)
}

/// The tracker's episode list over the first part's `first` episodes
/// with `extras` extras besides — `0`, `w.5`, `w.25`, `w.75` for `w` up
/// to `first`, started `rotate` along — each listed once or twice.
fn listing_with_extras(first: u32, extras: u32, rotate: usize, twice: bool) -> Vec<String> {
    let mut candidates = vec!["0".to_string()];
    for fraction in ["5", "25", "75"] {
        candidates.extend((0..=first).map(|w| format!("{w}.{fraction}")));
    }
    let len = candidates.len();
    candidates.rotate_left(rotate % len);
    let mut listed: Vec<String> = (1..=first).map(|n| n.to_string()).collect();
    for extra in candidates
        .into_iter()
        .take(usize::try_from(extras).unwrap_or(0))
    {
        if twice {
            listed.push(extra.clone());
        }
        listed.push(extra);
    }
    listed
}

/// Filing evidence built so the part-filing rules fire or nearly do:
/// Kitsu's chain of two to four parts with distinct ids and distinct
/// real start dates, the show one of them, summing to the entry's total
/// (or one off it), maybe running on past it; the mapped set standing
/// to the chain as [`Mapped`] says; the entry's list, when present,
/// explaining the surplus over the first part exactly or one short.
fn firing_filing_evidence() -> BoxedStrategy<(Evidence, u32)> {
    (2usize..=4)
        .prop_flat_map(|n| {
            (
                prop::collection::vec(1u32..=6, n),
                Just((0..5).map(|i| format!("p{i}")).collect::<Vec<_>>()).prop_shuffle(),
                prop::collection::btree_set(0u32..336, n)
                    .prop_map(|days| days.into_iter().collect::<Vec<_>>())
                    .prop_shuffle(),
                0..n,
                prop_oneof![
                    3 => Just(Mapped::Same),
                    3 => Just(Mapped::Outsider),
                    1 => Just(Mapped::Missing),
                    1 => Just(Mapped::Late),
                    1 => Just(Mapped::Undated),
                    2 => Just(Mapped::Empty),
                ],
                prop::collection::vec(1u32..=6, 0..=2),
                prop_oneof![4 => Just(0i8), 1 => Just(1), 1 => Just(-1)],
                prop_oneof![1 => Just(None), 2 => Just(Some(0u32)), 1 => Just(Some(1))],
                (0usize..16, any::<bool>(), any::<bool>()),
            )
        })
        .prop_map(
            |(counts, ids, days, ours, mapped, past, off, listing, (rotate, twice, finished))| {
                let mut parts: Vec<KitsuShow> = counts
                    .iter()
                    .zip(&ids)
                    .zip(&days)
                    .map(|((n, id), day)| KitsuShow {
                        id: id.clone(),
                        episodes: Some(*n),
                        start: Some(day_of_2020(*day)),
                        end: None,
                    })
                    .collect();
                let chain_len = parts.len();
                let sum: u32 = counts.iter().sum();
                let total = sum.saturating_add_signed(i32::from(off)).max(1);
                let other = (ours + 1) % chain_len;
                let mut sharing = parts.clone();
                for (i, n) in past.iter().enumerate() {
                    parts.push(KitsuShow {
                        id: format!("t{i}"),
                        episodes: Some(*n),
                        start: Some(day_of_2020(336 + u32::try_from(i).unwrap_or(0))),
                        end: None,
                    });
                }
                match mapped {
                    Mapped::Same => {}
                    Mapped::Outsider => sharing[other].id = "q0".into(),
                    Mapped::Missing => {
                        sharing.remove(other);
                    }
                    Mapped::Late => sharing.extend(parts.get(chain_len).cloned()),
                    Mapped::Undated => sharing[other].start = None,
                    Mapped::Empty => sharing.clear(),
                }
                let first = counts[0];
                let listed = listing.map(|short| {
                    let surplus = total.saturating_sub(first).saturating_sub(short);
                    listing_with_extras(first, surplus, rotate, twice)
                });
                let ev = Evidence {
                    kitsu: parts[ours].clone(),
                    entry: TrackerShow {
                        id: 9,
                        id_mal: None,
                        episodes: Some(total),
                        finished,
                        start: None,
                        end: None,
                    },
                    listed,
                    sharing,
                    parts,
                    ..Default::default()
                };
                (ev, total)
            },
        )
        .boxed()
}

/// A shared set summing exactly to the largest total there is, the show
/// among it, beside a run of Kitsu parts whose sum passes what `u32`
/// counts before it reaches that total.
fn overflowing_run_beside_a_shared_set() -> BoxedStrategy<Evidence> {
    (
        prop::collection::vec(1u32..=6, 1..=2),
        prop::collection::vec(u32::MAX - 6..u32::MAX, 2..=3),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(small, run, ours_first, finished)| {
            let small_sum: u32 = small.iter().sum();
            let mut counts = small;
            counts.push(u32::MAX - small_sum);
            let sharing: Vec<KitsuShow> = counts
                .iter()
                .enumerate()
                .map(|(i, n)| KitsuShow {
                    id: format!("s{i}"),
                    episodes: Some(*n),
                    start: Some(day_of_2020(u32::try_from(i).unwrap_or(0))),
                    end: None,
                })
                .collect();
            let kitsu = if ours_first {
                sharing[0].clone()
            } else {
                sharing[sharing.len() - 1].clone()
            };
            let mut parts: Vec<KitsuShow> = run
                .iter()
                .enumerate()
                .map(|(i, n)| KitsuShow {
                    id: format!("r{i}"),
                    episodes: Some(*n),
                    start: None,
                    end: None,
                })
                .collect();
            // The run may open with the show itself, carrying its own
            // count, so the overflow comes in the parts after it.
            if ours_first {
                parts.insert(0, kitsu.clone());
                parts[1].episodes = Some(u32::MAX);
            }
            Evidence {
                kitsu,
                entry: TrackerShow {
                    id: 9,
                    id_mal: None,
                    episodes: Some(u32::MAX),
                    finished,
                    start: None,
                    end: None,
                },
                sharing,
                parts,
                ..Default::default()
            }
        })
        .boxed()
}

/// Filing evidence with every id once in the parts and once in the
/// mapped set. The gatherer pushes a mapped entry only when the set
/// lacks its id, so the set never repeats one; Kitsu's sequel walk
/// could in principle revisit a part, but [`files_otherwise_model`]
/// compares the ids as sets, which a repeated part would make
/// disagree with the rule's count-based comparison, so the property
/// over that model keeps the parts unique too.
fn unique_ids((mut ev, total): (Evidence, u32)) -> (Evidence, u32) {
    for list in [&mut ev.parts, &mut ev.sharing] {
        let mut seen = std::collections::BTreeSet::new();
        list.retain(|p| seen.insert(p.id.clone()));
    }
    (ev, total)
}

/// Filing evidence whose parts sum past what `u32` counts, against the
/// largest total there is: no run of them sums to it exactly.
fn overflowing_filing_evidence() -> BoxedStrategy<(Evidence, u32)> {
    (
        filing_evidence(),
        prop::collection::vec(u32::MAX - 6..=u32::MAX, 2..4),
    )
        .prop_map(|((mut ev, _), counts)| {
            for (i, count) in counts.into_iter().enumerate() {
                ev.parts.insert(
                    i,
                    KitsuShow {
                        id: format!("p{i}"),
                        episodes: Some(count),
                        start: None,
                        end: None,
                    },
                );
            }
            ev.entry.episodes = Some(u32::MAX);
            (ev, u32::MAX)
        })
        .boxed()
}

/// `chain_files_otherwise` stated without its loop: the shortest run of
/// known, positive part counts whose exact sum reaches `total`; `None`
/// when no such run exists, because an unknown count comes first or the
/// parts run out short.
fn shortest_reaching_prefix(parts: &[KitsuShow], total: u32) -> Option<&[KitsuShow]> {
    let mut sum = 0u64;
    for (n, part) in parts.iter().enumerate() {
        let count = part.episodes.filter(|c| *c > 0)?;
        sum += u64::from(count);
        if sum >= u64::from(total) {
            return Some(&parts[..=n]);
        }
    }
    None
}

/// Whether Kitsu's chain files the entry otherwise than the mapped
/// set, stated from the rule's description rather than its code: the
/// shortest run of parts reaching the total, summed in `u64` so no sum
/// overflows, equals the total exactly, holds two parts or more, and
/// takes a set of ids other than the mapped set's.
fn files_otherwise_model(ev: &Evidence, total: u32) -> bool {
    let Some(run) = shortest_reaching_prefix(&ev.parts, total) else {
        return false;
    };
    let sum: u64 = run.iter().filter_map(|p| p.episodes).map(u64::from).sum();
    let run_ids: std::collections::BTreeSet<&str> = run.iter().map(|p| p.id.as_str()).collect();
    let mapped_ids: std::collections::BTreeSet<&str> =
        ev.sharing.iter().map(|k| k.id.as_str()).collect();
    sum == u64::from(total) && run.len() >= 2 && run_ids != mapped_ids
}

proptest! {
    /// Kitsu's chain files the entry otherwise than the mapped set
    /// exactly when the shortest run of parts reaching the entry's
    /// total sums to it exactly, holds two parts or more, and takes
    /// other entries than the mapped set ([`files_otherwise_model`]).
    #[test]
    fn the_chain_files_otherwise_exactly_when_its_reaching_run_is_not_the_mapped_set(
        (ev, total) in prop_oneof![
            4 => filing_evidence().prop_map(unique_ids),
            1 => overflowing_filing_evidence().prop_map(unique_ids),
        ]
    ) {
        prop_assert_eq!(chain_files_otherwise(&ev, total), files_otherwise_model(&ev, total));
    }

    /// A run of Kitsu parts that passes what `u32` counts reaches no
    /// total, so it rivals nothing: beside a shared set that sums to the
    /// entry exactly, the map is the shared set's, never a conflict.
    #[test]
    fn a_run_past_u32_beside_a_summing_shared_set_is_no_conflict(
        ev in overflowing_run_beside_a_shared_set()
    ) {
        let total = u32::MAX;
        let shared = shared_entry(&ev, total);
        prop_assert!(shared.is_some());
        prop_assert!(!chain_files_otherwise(&ev, total));
        let map = map_show(&ev);
        prop_assert_ne!(map.shape, Shape::Identity(Identity::Conflict));
        prop_assert_eq!(
            map,
            EpisodeMap { shape: Shape::SharedEntry, segments: shared.unwrap_or_default() }
        );
    }

    /// Where the shared-entry rule places the show and Kitsu's chain
    /// files the entry otherwise, the two explanations rival each
    /// other: the map is identity, for a conflict.
    #[test]
    fn a_shared_set_the_chain_files_otherwise_is_a_conflict((ev, total) in filing_evidence()) {
        // The rival only stands where the entry holds more than the show.
        if ev.kitsu.episodes.filter(|n| *n > 0 && *n < total).is_none() {
            return Ok(());
        }
        // The chain the map reads ends before a part that aired first.
        let mut ordered = ev.clone();
        let in_order = ordered
            .parts
            .windows(2)
            .position(|w| model::aired_first(w[1].start.as_deref(), w[0].start.as_deref()))
            .map_or(ordered.parts.len(), |i| i + 1);
        ordered.parts.truncate(in_order);
        if shared_entry(&ordered, total).is_some() && chain_files_otherwise(&ordered, total) {
            prop_assert_eq!(map_show(&ev), EpisodeMap::identity(Identity::Conflict));
        }
    }

    /// The split-parts rule yields at most its own placement and the
    /// extras rival to it, the placement first; the rival is the extras
    /// rule at the show's count where the show is the first part, and
    /// the first part's extras otherwise. A contradicted chain
    /// explains nothing of its own: it stands only as a rival to a
    /// shared set, and then alone.
    #[test]
    fn split_explanations_are_the_placement_and_at_most_one_rival(
        (ev, total) in filing_evidence(), shared in any::<bool>()
    ) {
        let Some(kitsu_count) = ev.kitsu.episodes.filter(|n| *n > 0 && *n < total) else {
            return Ok(());
        };
        let found = split_explanations(&ev, kitsu_count, total, shared);
        prop_assert!(found.len() <= 2);
        let placed = split_parts(&ev, total);
        match &placed {
            None => prop_assert!(found.is_empty()),
            Some((segments, contradicted)) => {
                if *contradicted && !shared {
                    prop_assert!(found.is_empty());
                } else {
                    prop_assert_eq!(found[0].0, Shape::SplitParts);
                    prop_assert_eq!(&found[0].1, segments);
                }
                if *contradicted {
                    prop_assert!(found.len() <= 1);
                } else {
                    // The rival is the extras rule read at the show's own
                    // count where the show is the first part, and from the
                    // first part otherwise.
                    let rival = if ev.parts.first().is_some_and(|p| p.id == ev.kitsu.id) {
                        extras(&ev, kitsu_count, total)
                    } else {
                        first_part_extras(&ev, total)
                    };
                    prop_assert_eq!(found.get(1).map(|(_, s)| s.clone()), rival);
                }
            }
        }
        if let Some((shape, _)) = found.get(1) {
            prop_assert_eq!(*shape, Shape::Extras);
        }
    }

    /// The extras rule read from the first part answers nothing without
    /// a first part, when the show is that part, or when its count is
    /// unknown, zero or not short of the entry's total; otherwise it is
    /// the extras rule with the first part's count.
    #[test]
    fn the_first_part_s_extras_are_the_extras_rule_at_its_count((ev, total) in filing_evidence()) {
        let got = first_part_extras(&ev, total);
        match ev.parts.first() {
            None => prop_assert_eq!(got, None),
            Some(first) if first.id == ev.kitsu.id => prop_assert_eq!(got, None),
            Some(first) => match first.episodes.filter(|n| *n > 0 && *n < total) {
                None => prop_assert_eq!(got, None),
                Some(count) => prop_assert_eq!(got, extras(&ev, count, total)),
            },
        }
    }

    /// Whatever evidence arrives, the first-part extras read answers
    /// without panicking, whatever counts the services sent.
    #[test]
    fn the_first_part_s_extras_answer_any_evidence(ev in any_evidence(), total in any_count()) {
        let _ = first_part_extras(&ev, total.unwrap_or(0));
    }

    /// Any text from the tracker's list reads as a number or as
    /// nothing, never a panic; a fraction that comes back is digits,
    /// at least one, never ending in a zero.
    #[test]
    fn a_listed_number_s_fraction_is_digits_without_trailing_zeros(text in ".{0,12}|[0-9.]{0,8}") {
        if let Some((_, Some(fraction))) = listed_number(&text) {
            prop_assert!(!fraction.is_empty());
            prop_assert!(fraction.chars().all(|c| c.is_ascii_digit()));
            prop_assert!(!fraction.ends_with('0'));
        }
    }

    /// `whole.digits` followed by trailing zeros reads back as `whole`
    /// and the digits; zeros alone after the point are no fraction, and
    /// a point with nothing after it is no number.
    #[test]
    fn a_listed_number_reads_back_without_its_trailing_zeros(
        whole: u32, digits in "([0-9]{0,5}[1-9])?", zeros in "0{0,3}"
    ) {
        let text = format!("{whole}.{digits}{zeros}");
        let want = if digits.is_empty() && zeros.is_empty() {
            None
        } else {
            Some((whole, (!digits.is_empty()).then_some(digits.as_str())))
        };
        prop_assert_eq!(listed_number(&text), want);
    }
}

/// Every shape a map can take, with any segments beside it.
fn any_shape() -> BoxedStrategy<Shape> {
    prop_oneof![
        Just(Shape::Identity(Identity::CountsAgree)),
        Just(Shape::Identity(Identity::CountUnknown)),
        Just(Shape::Identity(Identity::Unexplained)),
        Just(Shape::Identity(Identity::Conflict)),
        Just(Shape::Chain),
        Just(Shape::SharedEntry),
        Just(Shape::SplitParts),
        Just(Shape::Extras),
        Just(Shape::SpecialExtras),
    ]
    .boxed()
}

proptest! {
    /// A map is conclusive unless it is identity for want of evidence —
    /// a count unknown, a difference unexplained, or two explanations in
    /// conflict. Agreeing counts and every rule's shape are conclusive,
    /// whatever segments the map holds.
    #[test]
    fn a_map_is_inconclusive_exactly_for_identity_without_evidence(
        shape in any_shape(), ev in chain_evidence()
    ) {
        let map = EpisodeMap { shape, segments: map_show(&ev).segments };
        let want = match shape {
            Shape::Identity(Identity::CountsAgree) => true,
            Shape::Identity(Identity::CountUnknown) => false,
            Shape::Identity(Identity::Unexplained) => false,
            Shape::Identity(Identity::Conflict) => false,
            Shape::Chain => true,
            Shape::SharedEntry => true,
            Shape::SplitParts => true,
            Shape::Extras => true,
            Shape::SpecialExtras => true,
        };
        prop_assert_eq!(map.conclusive(), want);
    }
}

proptest! {
    /// Evidence that disagrees with itself settles nothing: when the
    /// show's own count is not the count its part carries in Kitsu's
    /// chain of parts (or in the mapped set), a map that files it
    /// still covers exactly the show's own episodes, or is identity.
    #[test]
    fn a_part_whose_count_is_not_the_show_s_own_files_nothing_past_the_show(
        (ev, ours) in parts_evidence(),
        own in 1u32..=60,
        shared in any::<bool>(),
    ) {
        let mut ev = ev;
        ev.kitsu.episodes = Some(own);
        if shared {
            ev.sharing = ev.parts.clone();
        }
        let part = ev.parts[ours].episodes;
        let map = map_show(&ev);
        if !matches!(map.shape, Shape::Identity(_)) {
            prop_assert_eq!(map.segments.last().map(|s| s.kitsu_to), Some(own));
        }
        if part != Some(own) {
            prop_assert!(
                !matches!(map.shape, Shape::SplitParts | Shape::SharedEntry),
                "a part counted {part:?} filed a show of {own}"
            );
        }
    }
}
