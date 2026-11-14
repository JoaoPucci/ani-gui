//! Which candidate parents a side story's parent check
//! (`franchise_walk_parents.rs`) reads: those it names as its parent
//! story, and the side stories and spin-offs it names that started
//! before it, as far as the walk's bound leaves room, in the order of
//! their ids as text, as the franchise graph reads its entries
//! (`franchise_walk_graph.rs`).

use std::collections::{BTreeMap, BTreeSet};

use super::alt::listed;
use super::{Lookup, MAX_LISTINGS};
use crate::meta::kitsu::FranchiseRelation;

/// The candidates the bound leaves room to read, in the order of their
/// ids as text: a candidate already `counted` toward the bound costs
/// nothing; the rest take what room is left. Whether any was cut.
fn within_bound(
    candidates: BTreeSet<String>,
    counted: &BTreeSet<String>,
) -> (BTreeSet<String>, bool) {
    let room = MAX_LISTINGS.saturating_sub(counted.len());
    let mut fresh = 0;
    let mut cut = false;
    let mut kept = BTreeSet::new();
    for id in candidates {
        if !counted.contains(&id) {
            if fresh == room {
                cut = true;
                continue;
            }
            fresh += 1;
        }
        kept.insert(id);
    }
    (kept, cut)
}

/// The candidates `anchor`'s check decides, by id with their starts:
/// those it names as its parent story, and those it names as its side
/// story or spin-off that started before it. Read from its own listing
/// and the `consulted` ones alone, so known before any candidate is.
pub(super) type Candidates<'l> = (
    BTreeMap<&'l str, Option<String>>,
    BTreeMap<&'l str, Option<String>>,
    Option<String>,
);

pub(super) fn candidates<'l>(
    anchor: &'l str,
    look: &'l Lookup<'_>,
    consulted: &BTreeSet<String>,
) -> Candidates<'l> {
    let start = |r: &FranchiseRelation| r.entry.as_ref().and_then(|e| e.start.clone());
    let own_start = look.entry(anchor, consulted).and_then(|e| e.start);
    let named = listed(look, anchor, &["parent_story"])
        .map(|r| (r.id.as_str(), start(r)))
        .collect();
    let earlier = listed(look, anchor, &["side_story", "spinoff"])
        .filter(|r| matches!((start(r), &own_start), (Some(a), Some(b)) if a < *b))
        .map(|r| (r.id.as_str(), start(r)))
        .collect();
    (named, earlier, own_start)
}

/// The candidates `anchor`'s check reads, beside the listings already
/// `counted` toward the bound, and whether the bound cut any.
pub(super) fn to_read(
    anchor: &str,
    look: &Lookup<'_>,
    consulted: &BTreeSet<String>,
    counted: &BTreeSet<String>,
) -> (BTreeSet<String>, bool) {
    let (named, earlier, _) = candidates(anchor, look, consulted);
    let all = named
        .keys()
        .chain(earlier.keys())
        .map(|id| (*id).to_string())
        .collect();
    within_bound(all, counted)
}
