//! The run laid out for the franchise page: straight stretches, forks
//! with their branches side by side, and loops as one block.

use std::collections::BTreeSet;

use serde::Serialize;

use super::graph::{link_graph, RunGraph};
use super::sequence::sequence;
use super::{WatchEntry, WatchOrder, WatchRun};

/// One stretch of the run as the franchise page lays it out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Segment {
    /// A straight stretch.
    Line {
        /// Its entries, in order.
        entries: Vec<WatchEntry>,
    },
    /// A fork: the entry it leaves from and its branches.
    Fork {
        /// The entry the branches leave from.
        from: WatchEntry,
        /// Each branch's chain, side by side.
        branches: Vec<Vec<WatchEntry>>,
    },
    /// A loop, as one block.
    Cycle {
        /// Its members, in order.
        entries: Vec<WatchEntry>,
    },
}

/// One branch from its first entry: the chain that follows it while
/// nothing forks or joins.
fn branch_from(
    graph: &RunGraph<'_>,
    head: &WatchEntry,
    placed: &mut BTreeSet<String>,
) -> Vec<WatchEntry> {
    let mut chain = Vec::new();
    let mut cur = Some(head);
    while let Some(e) = cur {
        if placed.contains(&e.id) || graph.cycle.contains(&e.id) || graph.before(&e.id).len() != 1 {
            break;
        }
        chain.push(e.clone());
        placed.insert(e.id.clone());
        let after = graph.after(&e.id);
        cur = if after.len() == 1 {
            Some(after[0])
        } else {
            None
        };
    }
    chain
}

/// The run as the franchise page lays it out: straight stretches, a
/// fork with its branches side by side, and a cycle as one block.
/// Release order is always one straight line.
#[must_use]
pub fn segments(run: &WatchRun, order: WatchOrder) -> Vec<Segment> {
    let seq = sequence(run, order);
    if order == WatchOrder::Released {
        return vec![Segment::Line {
            entries: seq.into_iter().cloned().collect(),
        }];
    }
    let graph = link_graph(run);
    let mut placed: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    let mut line: Vec<WatchEntry> = Vec::new();
    let flush = |line: &mut Vec<WatchEntry>, out: &mut Vec<Segment>| {
        if !line.is_empty() {
            out.push(Segment::Line {
                entries: std::mem::take(line),
            });
        }
    };
    for e in &seq {
        if placed.contains(&e.id) {
            continue;
        }
        if graph.cycle.contains(&e.id) {
            flush(&mut line, &mut out);
            let comp = graph.component.get(&e.id);
            let members: Vec<WatchEntry> = seq
                .iter()
                .filter(|x| graph.component.get(&x.id) == comp)
                .map(|x| (*x).clone())
                .collect();
            placed.extend(members.iter().map(|x| x.id.clone()));
            out.push(Segment::Cycle { entries: members });
            continue;
        }
        placed.insert(e.id.clone());
        line.push((*e).clone());
        let after = graph.after(&e.id);
        if after.len() > 1 {
            flush(&mut line, &mut out);
            let branches: Vec<Vec<WatchEntry>> = after
                .iter()
                .map(|n| branch_from(&graph, n, &mut placed))
                .filter(|b| !b.is_empty())
                .collect();
            // Branches that run straight into a loop are the loop's to
            // show: a fork with nothing left in it is not drawn.
            if !branches.is_empty() {
                out.push(Segment::Fork {
                    from: (*e).clone(),
                    branches,
                });
            }
        }
    }
    flush(&mut line, &mut out);
    out
}

/// How many entries share the loop `id` sits in, or 0 outside one.
#[must_use]
pub fn loop_size(run: &WatchRun, id: &str, order: WatchOrder) -> usize {
    segments(run, order)
        .iter()
        .find_map(|seg| match seg {
            Segment::Cycle { entries } if entries.iter().any(|e| e.id == id) => Some(entries.len()),
            _ => None,
        })
        .unwrap_or(0)
}
