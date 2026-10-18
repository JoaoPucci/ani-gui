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
        prop_assert_eq!(listed_number(&text), Some((whole, half)));
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
