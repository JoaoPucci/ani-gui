//! Where the walk places a side story alone, or an entry of a run of
//! one-offs (`franchise_walk_series.rs`): the parent stories its run's
//! members name outside the run, each checked with its own start
//! (`franchise_walk_parents.rs`), the earliest the one the walk moves
//! to.

use std::collections::BTreeSet;

use super::parents::parent_stories;
use super::rules::Run;
use super::{order_key, Lookup};

/// Where a side story alone, or a run of one-offs, is placed.
pub(super) struct Placed {
    /// The parent story to walk from: the earliest any member names.
    pub(super) root: Option<String>,
    /// The anchor's parents to name: its own, else its run's.
    pub(super) parents: Vec<String>,
    /// Every candidate listing read to decide.
    pub(super) read: BTreeSet<String>,
}

/// The parent stories the members of `own` name outside it (each
/// checked with its own start), the earliest the one to walk from; the
/// anchor's own are the ones it is placed under, else every member's.
pub(super) fn place(anchor: &str, own: &Run, look: &Lookup<'_>) -> Result<Placed, Vec<String>> {
    let mut read = BTreeSet::new();
    let mut unread = BTreeSet::new();
    let mut mine = Vec::new();
    let mut all = BTreeSet::new();
    for m in own.members.keys() {
        match parent_stories(m, look, &own.consulted) {
            Ok(found) => {
                read.extend(found.read);
                let outside: Vec<String> = found
                    .parents
                    .into_iter()
                    .filter(|p| !own.members.contains_key(p))
                    .collect();
                if m == anchor {
                    mine.clone_from(&outside);
                }
                all.extend(outside);
            }
            Err(need) => unread.extend(need),
        }
    }
    if !unread.is_empty() {
        return Err(unread.into_iter().collect());
    }
    let mut consulted = own.consulted.clone();
    consulted.extend(read.iter().cloned());
    let key = |p: &String| {
        let start = look.entry(p, &consulted).and_then(|e| e.start);
        order_key(start.as_deref(), p)
    };
    let mut all: Vec<String> = all.into_iter().collect();
    all.sort_by_key(key);
    mine.sort_by_key(key);
    Ok(Placed {
        root: all.first().cloned(),
        parents: if mine.is_empty() { all } else { mine },
        read,
    })
}
