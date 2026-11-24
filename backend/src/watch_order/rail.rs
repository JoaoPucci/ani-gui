//! Why an entry of the run has no number: a branch, a loop, or an entry
//! that only leads into a join.

use serde::Serialize;

use super::graph::link_graph;
use super::place::linked_position;
use super::{WatchEntry, WatchOrder, WatchRun};

/// Why an unnumbered entry has no number, for the line under its card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PathNote {
    /// It sits in a loop.
    Cycle,
    /// It starts a branch after a fork.
    Branch,
    /// It only leads into `entry`, a join.
    LeadsInto {
        /// The entry it leads into.
        entry: WatchEntry,
    },
}

/// Why `id` has no number, or none (numbered, released order, or no
/// reason to name).
#[must_use]
pub fn path_note(run: &WatchRun, id: &str, order: WatchOrder) -> Option<PathNote> {
    if order == WatchOrder::Released {
        return None;
    }
    let graph = link_graph(run);
    if graph.cycle.contains(id) {
        return Some(PathNote::Cycle);
    }
    if linked_position(&graph, id, &run.name_id).is_some() {
        return None;
    }
    let before = graph.before(id);
    if before.len() == 1 && graph.after(&before[0].id).len() > 1 {
        return Some(PathNote::Branch);
    }
    let after = graph.after(id);
    if before.is_empty() && after.len() == 1 {
        return Some(PathNote::LeadsInto {
            entry: after[0].clone(),
        });
    }
    None
}
