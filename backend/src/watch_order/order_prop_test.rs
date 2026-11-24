//! Properties of the run's ordering over generated runs: the link graph,
//! the one-list sequence, the segments, an entry's place and its note.

use std::collections::BTreeMap;

use proptest::prelude::*;

use super::graph::link_graph;
use super::place::placement;
use super::rail::{path_note, PathNote};
use super::run::run_of;
use super::segments::{segments, Segment};
use super::sequence::sequence;
use super::strategies::franchise_arb;
use super::{WatchOrder, WatchRun};

use WatchOrder::{Linked, Released};

/// A run from a generated payload, and an order.
fn any_run() -> BoxedStrategy<(WatchRun, WatchOrder)> {
    (
        franchise_arb(),
        prop::sample::select(&[Linked, Released][..]),
    )
        .prop_filter_map("a run", |(f, order)| run_of(&f).map(|r| (r, order)))
        .boxed()
}

fn counts<'a>(ids: impl IntoIterator<Item = &'a str>) -> BTreeMap<&'a str, usize> {
    let mut out = BTreeMap::new();
    for id in ids {
        *out.entry(id).or_default() += 1;
    }
    out
}

proptest! {
    #[test]
    fn the_graph_keeps_only_the_runs_own_links_and_a_cycle_is_a_component_of_several((run, _) in any_run()) {
        let g = link_graph(&run);
        for (a, list) in &g.next {
            for b in list {
                prop_assert!(run.links.contains(&(a.clone(), b.id.clone())));
                prop_assert!(g.before(&b.id).iter().any(|p| &p.id == a));
            }
        }
        for id in &g.cycle {
            let c = g.component[id];
            prop_assert!(g.component.values().filter(|x| **x == c).count() > 1);
        }
    }

    #[test]
    fn the_sequence_lists_every_member_once((run, order) in any_run()) {
        let seq = sequence(&run, order);
        let want = counts(run.entries.iter().map(|e| e.id.as_str()));
        prop_assert_eq!(counts(seq.iter().map(|e| e.id.as_str())), want);
    }

    #[test]
    fn the_segments_place_every_member_once_and_release_order_is_one_line((run, order) in any_run()) {
        let segs = segments(&run, order);
        let mut placed = Vec::new();
        for s in &segs {
            match s {
                Segment::Line { entries } | Segment::Cycle { entries } => placed.extend(entries.iter().map(|e| e.id.as_str())),
                Segment::Fork { branches, .. } => placed.extend(branches.iter().flatten().map(|e| e.id.as_str())),
            }
        }
        prop_assert_eq!(counts(placed), counts(run.entries.iter().map(|e| e.id.as_str())));
        if order == Released {
            let seq: Vec<&str> = sequence(&run, order).iter().map(|e| e.id.as_str()).collect();
            let [Segment::Line { entries }] = segs.as_slice() else {
                return Err(TestCaseError::fail("release order is one line"));
            };
            prop_assert_eq!(entries.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), seq);
        }
    }

    #[test]
    fn a_place_names_members_numbers_by_release_and_a_numbered_entry_has_no_note((run, order) in any_run()) {
        let line: Vec<String> = sequence(&run, Released).iter().map(|e| e.id.clone()).collect();
        for e in &run.entries {
            let p = placement(&run, &e.id, order);
            for n in p.before.iter().chain(&p.after) {
                prop_assert!(run.entries.iter().any(|m| m.id == n.id));
            }
            let note = path_note(&run, &e.id, order);
            if order == Released {
                let at = line.iter().position(|x| *x == e.id).expect("a member");
                prop_assert_eq!(p.position, Some(at + 1));
                prop_assert_eq!(p.total, Some(run.entries.len()));
                prop_assert_eq!(note, None);
            } else {
                prop_assert_eq!(p.in_cycle, matches!(note, Some(PathNote::Cycle)));
                if p.position.is_some() {
                    prop_assert_eq!(note, None);
                }
            }
        }
    }
}
