//! The anchor's own chain as the response serves it
//! (`franchise_walk_chain.rs` cuts it): its titles and seams in start
//! order, then numeric id, and the prequel and sequel links between
//! them, none between two versions of one another.

use std::collections::BTreeSet;

use super::around::entries;
use super::chain::linked;
use super::links::links_among;
use super::rules::Run;
use super::{Chain, Lookup};

/// The anchor's own chain, as served.
pub(super) fn served(run: &Run, look: &Lookup<'_>) -> Option<Chain> {
    let ids = run.chain.as_ref()?;
    let all: BTreeSet<String> = ids.own.union(&ids.seams).cloned().collect();
    Some(Chain {
        entries: entries(&all, run, look),
        links: links_among(&all, look)
            .into_iter()
            .filter(|(a, b)| linked(look, a, b))
            .collect(),
        seams: entries(&ids.seams, run, look)
            .into_iter()
            .map(|e| e.id)
            .collect(),
    })
}
