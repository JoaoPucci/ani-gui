//! proptest strategies for the watch-order properties: runs of a few
//! entries with clashing and missing dates, and links that may name
//! strangers, repeat, or point an entry at itself, so a property covers
//! what a malformed payload could carry (the renderer's
//! `testing/arbitraries.ts`, ported). Every strategy is boxed, for the
//! Windows test build's PDB limit (see `meta/kitsu_relations_prop_test.rs`).

use std::collections::BTreeMap;

use proptest::prelude::*;

use super::{Franchise, HistoryRow, RunLink, WatchEntry};

const DATES: [&str; 5] = [
    "2001-04-01",
    "2004-10-05",
    "2004-10-05",
    "2012-07-01",
    "2030-01-01",
];

/// An entry with id `id`.
pub(crate) fn entry_arb(id: String) -> BoxedStrategy<WatchEntry> {
    let title_id = id.clone();
    (
        0..4usize,
        prop::option::of(prop::sample::select(&["TV", "movie", "OVA"][..])),
        prop::option::of(prop::sample::select(&DATES[..])),
        prop::option::of(1u32..=30),
        prop::option::of(prop::sample::select(&["finished", "current"][..])),
    )
        .prop_map(move |(t, subtype, start, episodes, status)| WatchEntry {
            id: id.clone(),
            title: ["A", "A: B", "A: C", &format!("T{title_id}")][t].to_string(),
            subtype: subtype.map(Into::into),
            start: start.map(Into::into),
            episodes,
            status: status.map(Into::into),
            poster: None,
            cover: None,
        })
        .boxed()
}

/// Between `min` and `max` entries with distinct numeric ids.
pub(crate) fn entries_arb(
    min: usize,
    max: usize,
    ids: std::ops::RangeInclusive<u32>,
) -> BoxedStrategy<Vec<WatchEntry>> {
    prop::collection::btree_set(ids, min..=max)
        .prop_flat_map(|set| {
            let ids: Vec<u32> = set.into_iter().collect();
            ids.into_iter()
                .map(|i| entry_arb(i.to_string()))
                .collect::<Vec<_>>()
        })
        .prop_shuffle()
        .boxed()
}

/// Up to `max` links between ids of `ids` (which must not be empty).
pub(crate) fn links_arb(ids: Vec<String>, max: usize) -> BoxedStrategy<Vec<RunLink>> {
    let id = prop::sample::select(ids);
    prop::collection::vec((id.clone(), id), 0..=max).boxed()
}

/// A payload of any shape the parser could hand over, malformed links
/// and parents included.
pub(crate) fn franchise_arb() -> BoxedStrategy<Franchise> {
    entries_arb(0, 7, 1..=60)
        .prop_flat_map(|entries| {
            let mut ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
            let name_from = ids.first().cloned().unwrap_or_else(|| "0".into());
            let name_from = if ids.is_empty() {
                Just(name_from).boxed()
            } else {
                prop::sample::select(ids.clone()).boxed()
            };
            ids.extend(["999".to_string(), "0".to_string()]);
            let pick = prop::sample::select(ids);
            (
                Just(entries),
                pick.clone(),
                name_from,
                prop::collection::vec((pick.clone(), pick.clone()), 0..=10),
                prop::collection::btree_map(
                    pick.clone(),
                    prop::collection::vec(pick.clone(), 0..=3),
                    0..=2,
                ),
                any::<bool>(),
            )
        })
        .prop_map(
            |(entries, anchor, name_from, links, parents, complete)| Franchise {
                anchor,
                name: "F".into(),
                name_from,
                entries,
                links,
                versions: Vec::new(),
                films: Vec::new(),
                sides: Vec::new(),
                recaps: Vec::new(),
                parents,
                complete,
                chain: None,
            },
        )
        .boxed()
}

/// A payload whose run has titles around it too: other versions, films,
/// side stories and recaps, with ids apart from the run's and from each
/// other's. The anchor is a run member or one of those titles.
pub(crate) fn franchise_around_arb() -> BoxedStrategy<Franchise> {
    (
        entries_arb(1, 6, 1..=60),
        entries_arb(0, 8, 100..=140),
        prop::collection::vec(0..4usize, 8),
    )
        .prop_flat_map(|(entries, around, kinds)| {
            let ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
            let anchors: Vec<String> = ids
                .iter()
                .cloned()
                .chain(around.iter().map(|e| e.id.clone()))
                .collect();
            let id = prop::sample::select(ids);
            (
                Just((entries, around, kinds)),
                prop::sample::select(anchors),
                id.clone(),
                prop::collection::vec((id.clone(), id), 0..=8),
                any::<bool>(),
            )
        })
        .prop_map(
            |((entries, around, kinds), anchor, name_from, links, complete)| {
                let of = |k: usize| -> Vec<WatchEntry> {
                    around
                        .iter()
                        .zip(&kinds)
                        .filter(|(_, &i)| i == k)
                        .map(|(e, _)| e.clone())
                        .collect()
                };
                Franchise {
                    anchor,
                    name: "F".into(),
                    name_from,
                    entries: entries.clone(),
                    links,
                    versions: of(0),
                    films: of(1),
                    sides: of(2),
                    recaps: of(3),
                    parents: BTreeMap::new(),
                    complete,
                    chain: None,
                }
            },
        )
        .boxed()
}

/// Up to six history rows naming `ids` (or a stranger, or nothing).
pub(crate) fn rows_arb(ids: Vec<String>) -> BoxedStrategy<Vec<HistoryRow>> {
    let mut named = ids;
    named.push("0".into());
    let kitsu = prop::option::of(prop::sample::select(named));
    let ep = prop::sample::select(&["1", "2", "12", "30", "0.5", "x"][..]);
    prop::collection::vec(
        (kitsu, ep).prop_map(|(kitsu_id, ep_no)| HistoryRow {
            kitsu_id,
            ep_no: ep_no.into(),
        }),
        0..=6,
    )
    .boxed()
}
