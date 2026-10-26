//! Property coverage for the episode map: whatever evidence a rule
//! fires on, the segments cover Kitsu's episodes once each and in
//! order, every episode lands inside its entry, nothing moves backwards,
//! and a write completes an entry only at the entry's own last episode.

use super::*;
use crate::account::episode_map::{EpisodeMap, Evidence, KitsuShow, Shape, Target, TrackerShow};
use proptest::prelude::*;

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
fn chain_evidence() -> impl Strategy<Value = Evidence> {
    prop::collection::vec((1u32..=60, any::<bool>()), 2..=5).prop_map(|segments| {
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
}

/// A list of Kitsu's episodes plus extras after some of them, each
/// extra once or twice (as AniList lists a subbed and a dubbed track).
fn extras_evidence() -> impl Strategy<Value = (Evidence, Vec<u32>)> {
    (1u32..=40, any::<bool>()).prop_flat_map(|(count, finished)| {
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
}

/// Two to four Kitsu parts under one entry, one of them the show mapped.
fn parts_evidence() -> impl Strategy<Value = (Evidence, usize)> {
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
fn any_count() -> impl Strategy<Value = Option<u32>> {
    prop_oneof![
        Just(None),
        (0u32..=40).prop_map(Some),
        (u32::MAX - 40..=u32::MAX).prop_map(Some),
        any::<u32>().prop_map(Some),
    ]
}

/// A date as any source could send it: real, partial, unreal or absent.
fn any_date() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        Just(Some("2001-03-05".to_string())),
        Just(Some("2009-10".to_string())),
        Just(Some("2005".to_string())),
        Just(Some("2001-02-31".to_string())),
        "[0-9-]{0,12}".prop_map(Some),
    ]
}

fn any_tracker_show(id: u32) -> impl Strategy<Value = TrackerShow> {
    (any_count(), any::<bool>(), any_date(), any_date()).prop_map(
        move |(episodes, finished, start, end)| TrackerShow {
            id,
            id_mal: None,
            episodes,
            finished,
            start,
            end,
        },
    )
}

fn any_kitsu_show(id: &'static str) -> impl Strategy<Value = KitsuShow> {
    (any_count(), any_date(), any_date()).prop_map(move |(episodes, start, end)| KitsuShow {
        id: id.into(),
        episodes,
        start,
        end,
    })
}

/// Evidence as the gatherer could hand it over, every field free.
fn any_evidence() -> impl Strategy<Value = Evidence> {
    (
        any_kitsu_show("k"),
        any_tracker_show(1),
        prop::collection::vec(any_tracker_show(2), 0..4),
        any::<bool>(),
        proptest::option::of(prop::collection::vec("[0-9.]{0,5}", 0..6)),
        prop::collection::vec(any_kitsu_show("s"), 0..4),
        prop::collection::vec(any_kitsu_show("p"), 0..4),
    )
        .prop_map(|(kitsu, entry, mut chain, claim, listed, sharing, parts)| {
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
                parts,
            }
        })
}

proptest! {
    /// Whatever evidence arrives, the rules answer: they never panic,
    /// whatever counts or dates the services sent. A map they answer
    /// with a small count holds the invariants every shape shares.
    #[test]
    fn any_evidence_maps_without_panicking(ev in any_evidence()) {
        let map = map_show(&ev);
        if let (Shape::Identity(_), true) = (map.shape, map.segments.is_empty()) {
            return Ok(());
        }
        if let Some(count) = ev.kitsu.episodes.filter(|n| *n <= 500) {
            check_mapped(&map, count)?;
        }
    }
}

/// A count as the part-filing rules see one: mostly small so parts sum
/// to a total often, sometimes unknown or zero, sometimes near the top
/// of the range where a sum overflows.
fn part_count() -> impl Strategy<Value = Option<u32>> {
    prop_oneof![
        1 => Just(None),
        1 => Just(Some(0)),
        6 => (1u32..=6).prop_map(Some),
        1 => (u32::MAX - 6..=u32::MAX).prop_map(Some),
    ]
}

/// An entry total to set against those parts.
fn part_total() -> impl Strategy<Value = u32> {
    prop_oneof![6 => 1u32..=20, 1 => u32::MAX - 6..=u32::MAX]
}

/// A Kitsu id from a pool small enough that parts, the mapped set and
/// the show itself name the same entries often.
fn pooled_id() -> impl Strategy<Value = String> {
    (0u8..5).prop_map(|i| format!("p{i}"))
}

fn pooled_kitsu() -> impl Strategy<Value = KitsuShow> {
    (pooled_id(), part_count(), any::<bool>()).prop_map(|(id, episodes, dated)| KitsuShow {
        id,
        episodes,
        start: dated.then(|| "2020-01-01".to_string()),
        end: None,
    })
}

/// Evidence where Kitsu's parts and the entries mapped to the tracker's
/// entry overlap: the show one of the parts or not, the mapped set a
/// mix of the parts and others, the entry's list of numbers present or
/// not.
fn filing_evidence() -> impl Strategy<Value = (Evidence, u32)> {
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
}

/// Filing evidence whose parts sum past what `u32` counts, against the
/// largest total there is: no run of them sums to it exactly.
fn overflowing_filing_evidence() -> impl Strategy<Value = (Evidence, u32)> {
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

proptest! {
    /// Kitsu's chain files the entry otherwise than the mapped set
    /// exactly when the shortest run of parts reaching the entry's
    /// total sums to it exactly, holds two parts or more, and is not
    /// the mapped set: a different number of entries, or a mapped one
    /// the run does not take.
    #[test]
    fn the_chain_files_otherwise_exactly_when_its_reaching_run_is_not_the_mapped_set(
        (ev, total) in prop_oneof![4 => filing_evidence(), 1 => overflowing_filing_evidence()]
    ) {
        let want = shortest_reaching_prefix(&ev.parts, total).is_some_and(|run| {
            let sum: u64 = run.iter().filter_map(|p| p.episodes).map(u64::from).sum();
            let differs = run.len() != ev.sharing.len()
                || ev.sharing.iter().any(|k| !run.iter().any(|p| p.id == k.id));
            sum == u64::from(total) && run.len() >= 2 && differs
        });
        prop_assert_eq!(chain_files_otherwise(&ev, total), want);
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
        if shared_entry(&ev, total).is_some() && chain_files_otherwise(&ev, total) {
            prop_assert_eq!(map_show(&ev), EpisodeMap::identity(Identity::Conflict));
        }
    }

    /// The split-parts rule yields at most its own placement and the
    /// extras rival to it, the placement first. A contradicted chain
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
fn any_shape() -> impl Strategy<Value = Shape> {
    prop_oneof![
        Just(Shape::Identity(Identity::CountsAgree)),
        Just(Shape::Identity(Identity::CountUnknown)),
        Just(Shape::Identity(Identity::Unexplained)),
        Just(Shape::Identity(Identity::Conflict)),
        Just(Shape::Chain),
        Just(Shape::SharedEntry),
        Just(Shape::SplitParts),
        Just(Shape::Extras),
    ]
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
        };
        prop_assert_eq!(map.conclusive(), want);
    }
}
