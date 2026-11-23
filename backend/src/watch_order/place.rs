//! One entry's place in its run: its number, the run's length where the
//! order is one line, and the entries just before and after it.

use serde::Serialize;

use super::by_start::sort_by_start;
use super::graph::{link_graph, RunGraph};
use super::{WatchEntry, WatchOrder, WatchRun};

/// An entry's place in its run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Placement {
    /// Place counted from the run's first entry, or none where unnumbered.
    pub position: Option<usize>,
    /// Run length, only where the order is one line.
    pub total: Option<usize>,
    /// The entries just before it.
    pub before: Vec<WatchEntry>,
    /// The entries just after it.
    pub after: Vec<WatchEntry>,
    /// Whether it sits in a cycle.
    pub in_cycle: bool,
}

/// The run's members by start date.
#[must_use]
pub fn released_line(run: &WatchRun) -> Vec<&WatchEntry> {
    let mut line: Vec<&WatchEntry> = run.entries.iter().collect();
    sort_by_start(&mut line);
    line
}

/// Count along the anchor's own path back to the run's first entry,
/// taking the earliest-starting prequel at a join. A path that stops at
/// a cycle, passes a fork, or ends at an entry other than the run's
/// first is unnumbered: numbering it would give a second "01".
#[must_use]
pub fn linked_position(graph: &RunGraph<'_>, id: &str, first_id: &str) -> Option<usize> {
    let mut cur = id.to_string();
    // A path longer than the run has gone round: unnumbered.
    for steps in 0..=graph.prev.len() {
        if graph.cycle.contains(&cur) {
            return None;
        }
        let Some(pick) = graph.before(&cur).first() else {
            return (cur == first_id).then_some(steps + 1);
        };
        if graph.after(&pick.id).len() > 1 {
            return None;
        }
        cur.clone_from(&pick.id);
    }
    None
}

/// Whether the linked order is one line: no cycle, no fork, no join.
fn is_line(graph: &RunGraph<'_>) -> bool {
    graph.cycle.is_empty()
        && graph
            .prev
            .values()
            .chain(graph.next.values())
            .all(|l| l.len() <= 1)
}

fn owned(list: &[&WatchEntry]) -> Vec<WatchEntry> {
    list.iter().map(|e| (*e).clone()).collect()
}

/// `id`'s place in `run`, in `order`.
#[must_use]
pub fn placement(run: &WatchRun, id: &str, order: WatchOrder) -> Placement {
    if order == WatchOrder::Released {
        let line = released_line(run);
        let at = line.iter().position(|e| e.id == id);
        // An id outside the run reads as the renderer's `findIndex`
        // miss did: at -1, numbered 0.
        let (position, before, after) = match at {
            Some(at) => (
                at + 1,
                &line[at.saturating_sub(1)..at],
                line.get(at + 1..at + 2),
            ),
            None => (0, &line[..line.len().saturating_sub(1)], line.get(..1)),
        };
        return Placement {
            position: Some(position),
            total: Some(line.len()),
            before: owned(before),
            after: owned(after.unwrap_or(&[])),
            in_cycle: false,
        };
    }
    let graph = link_graph(run);
    Placement {
        position: linked_position(&graph, id, &run.name_id),
        total: is_line(&graph).then_some(run.entries.len()),
        before: owned(graph.before(id)),
        after: owned(graph.after(id)),
        in_cycle: graph.cycle.contains(id),
    }
}
