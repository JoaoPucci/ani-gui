//! Going back from an entry toward the franchise's root
//! (`franchise_walk_root.rs`): which of its prequels are followed, and
//! which are passed over.

use super::alt::{components, linked};
use super::graph::Graph;
use super::{order_key, Lookup};

/// The prequels of `at` in the franchise to follow back, and those it
/// passes over: of each group that are versions of one another, the one
/// in `at`'s format, else the earliest, else the lowest id, is followed.
pub(super) fn prequels(at: &str, look: &Lookup<'_>, g: &Graph) -> (Vec<String>, Vec<String>) {
    let ids: Vec<String> = linked(look, at, "prequel", &g.consulted)
        .into_iter()
        .filter(|p| g.entries.contains(p))
        .collect();
    let format = look.entry(at, &g.consulted).and_then(|e| e.subtype);
    let key = |id: &String| {
        let e = look.entry(id, &g.consulted);
        let same = format.is_some() && e.as_ref().and_then(|e| e.subtype.clone()) == format;
        let start = e.and_then(|e| e.start);
        (!same, order_key(start.as_deref(), id))
    };
    let (mut chosen, mut passed) = (Vec::new(), Vec::new());
    for group in components(look, &ids) {
        // A group is never empty: each holds the id it was started from.
        if let Some(winner) = group.iter().min_by_key(|id| key(id)).cloned() {
            passed.extend(group.into_iter().filter(|g| *g != winner));
            chosen.push(winner);
        }
    }
    (chosen, passed)
}
