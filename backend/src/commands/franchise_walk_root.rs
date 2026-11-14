//! The franchise's main story, the same from every page of it.
//!
//! A walk keeps its root whatever it meets (rule 3 never leaves the
//! root out), so a walk from a film, an OVA or a later season can make
//! a run of a line the rest of the franchise treats as another version
//! of it. The main story is therefore the walk from the franchise's
//! root, not from the anchor, decided over the franchise as a whole:
//!
//! - **The franchise** is every entry joined to the run first walked to
//!   by prequel and sequel links, a link counted from either side: an
//!   entry a listing names as its prequel has that listing's entry as a
//!   sequel, though its own listing does not say so.
//! - **Its ends** are the entries no entry of it follows. Going back
//!   from them along the prequels, of prequels that are versions of one
//!   another one is followed, as rule 2 keeps one: the one in the
//!   format of the entry that lists them, else the earliest, else the
//!   lowest id. The others are passed over.
//! - **Its root** is the earliest (by start, then the lowest id) entry
//!   reached that has no prequel and was not passed over anywhere: the
//!   choice made where two lines meet decides, so a line that loses
//!   there does not offer its own first entry. Where every way back
//!   leads into a loop, the earliest entry reached that was not passed
//!   over stands for the root.
//! - **The main story** is the walk from the root, the franchise's links
//!   counted from either side.
//!
//! Every page of a franchise reads the same franchise, so every page
//! answers with the same main story, and so with the same name and
//! front entry. A link Kitsu lists on one side only is seen from that
//! side alone: a page that cannot see an entry cannot count it.
//!
//! Every listing read counts toward the walk's bound; an entry past it
//! is not read, and the answer is incomplete
//! (`franchise_walk_graph.rs`).

use std::collections::BTreeSet;

use super::alt::{components, linked};
use super::graph::{graph, Graph};
use super::walking::{walk_with, Run};
use super::{order_key, Lookup};

/// The prequels of `at` in the franchise to follow back, and those it
/// passes over: of each group that are versions of one another, the one
/// in `at`'s format, else the earliest, else the lowest id, is followed.
fn prequels(at: &str, look: &Lookup<'_>, g: &Graph) -> (Vec<String>, Vec<String>) {
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
        let Some(winner) = group.iter().min_by_key(|id| key(id)).cloned() else {
            continue;
        };
        passed.extend(group.into_iter().filter(|g| *g != winner));
        chosen.push(winner);
    }
    (chosen, passed)
}

/// The franchise's root.
fn root(look: &Lookup<'_>, g: &Graph) -> Option<String> {
    let follows = |e: &String| {
        linked(look, e, "sequel", &g.consulted)
            .iter()
            .any(|s| g.entries.contains(s))
    };
    let mut frontier: Vec<String> = g.entries.iter().filter(|e| !follows(e)).cloned().collect();
    if frontier.is_empty() {
        frontier = g.entries.iter().cloned().collect();
    }
    let mut reached: BTreeSet<String> = frontier.iter().cloned().collect();
    let mut passed = BTreeSet::new();
    let mut roots = BTreeSet::new();
    while let Some(at) = frontier.pop() {
        let (chosen, lost) = prequels(&at, look, g);
        passed.extend(lost);
        if chosen.is_empty() {
            roots.insert(at.clone());
        }
        for p in chosen {
            if reached.insert(p.clone()) {
                frontier.push(p);
            }
        }
    }
    roots.retain(|r| !passed.contains(r));
    reached.retain(|r| !passed.contains(r));
    let candidates = if roots.is_empty() { &reached } else { &roots };
    candidates
        .iter()
        .min_by_key(|r| {
            let start = look.entry(r, &g.consulted).and_then(|e| e.start);
            order_key(start.as_deref(), r)
        })
        .cloned()
}

/// The main story `first` settles to: the walk from the franchise's
/// root, every listing read on the way counted, and the franchise's
/// entries (every entry joined to it by prequel and sequel links), so
/// those it leaves out are listed around it whichever page asked.
///
/// # Errors
/// The listings to read before it can go on.
pub(super) fn settle(
    first: Run,
    look: &Lookup<'_>,
) -> Result<(Run, BTreeSet<String>), Vec<String>> {
    let g = graph(&first, look)?;
    let complete = first.complete && g.complete;
    let Some(root) = root(look, &g) else {
        let run = Run {
            consulted: g.consulted,
            complete,
            ..first
        };
        return Ok((run, g.entries));
    };
    let main = walk_with(&root, look, g.consulted.clone(), g.consulted)?;
    let run = Run {
        complete: complete && main.complete,
        ..main
    };
    Ok((run, g.entries))
}

#[cfg(test)]
#[path = "franchise_walk_root_test.rs"]
mod tests;
