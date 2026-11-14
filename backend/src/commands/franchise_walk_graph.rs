//! The franchise around a run (`franchise_walk_root.rs`): every entry
//! joined to its members by prequel and sequel links, a link counted
//! from either side, and every listing read on the way. Every listing
//! read counts toward the walk's bound; an entry past it is not read,
//! and the franchise is incomplete.

use std::collections::BTreeSet;

use super::alt::linked;
use super::walking::Run;
use super::{Lookup, MAX_LISTINGS};

/// The franchise around a run: its entries and every listing read.
pub(super) struct Graph {
    pub(super) entries: BTreeSet<String>,
    pub(super) consulted: BTreeSet<String>,
    pub(super) complete: bool,
}

/// What `at` is linked to by prequel or sequel, from either side.
pub(super) fn neighbours(
    at: &str,
    look: &Lookup<'_>,
    consulted: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut out = linked(look, at, "prequel", consulted);
    out.extend(linked(look, at, "sequel", consulted));
    out
}

/// Every entry joined to `run`'s members by prequel and sequel links.
///
/// # Errors
/// The listings to read before it can go on.
pub(super) fn graph(run: &Run, look: &Lookup<'_>) -> Result<Graph, Vec<String>> {
    let mut g = Graph {
        entries: run.members.keys().cloned().collect(),
        consulted: run.consulted.clone(),
        complete: true,
    };
    let mut frontier: Vec<String> = g.entries.iter().cloned().collect();
    while !frontier.is_empty() {
        let mut next = BTreeSet::new();
        for at in &frontier {
            for n in neighbours(at, look, &g.consulted) {
                if !g.entries.contains(&n) {
                    next.insert(n);
                }
            }
        }
        let mut need = Vec::new();
        let mut added = Vec::new();
        for n in next {
            // A listing still to read counts toward the bound as one
            // read: the IO loop reads every listing asked for at once.
            if !g.consulted.contains(&n) && g.consulted.len() + need.len() >= MAX_LISTINGS {
                g.complete = false;
                continue;
            }
            if look.listing(&n).is_none() {
                need.push(n);
                continue;
            }
            g.consulted.insert(n.clone());
            added.push(n);
        }
        if !need.is_empty() {
            return Err(need);
        }
        g.entries.extend(added.iter().cloned());
        frontier = added;
    }
    Ok(g)
}
