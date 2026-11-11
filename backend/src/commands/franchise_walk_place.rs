//! Where the walk places a side story alone, or an entry of a run of
//! one-offs (`franchise_walk_series.rs`): the parent stories its run's
//! members name outside the run, each checked with its own start
//! (`franchise_walk_parents.rs`), the earliest the one the walk moves
//! to.

use std::collections::{BTreeMap, BTreeSet};

use super::parents::parent_stories;
use super::rules::Run;
use super::{order_key, Lookup};

/// Where a side story alone, or a run of one-offs, is placed.
pub(super) struct Placed {
    /// The parent story to walk from: the earliest any member names.
    pub(super) root: Option<String>,
    /// The parent stories the anchor names outside its run, earliest first.
    mine: Vec<String>,
    /// The parent stories any member names outside the run, earliest first.
    all: Vec<String>,
    /// Every candidate listing read to decide.
    pub(super) read: BTreeSet<String>,
}

impl Placed {
    /// The anchor's parents among the `members` of the run the walk
    /// moved to: its own there, else its run's there, which hold the
    /// root, so the list is never empty.
    pub(super) fn parents_in(self, members: &BTreeMap<String, Option<String>>) -> Vec<String> {
        let within = |list: Vec<String>| -> Vec<String> {
            list.into_iter()
                .filter(|p| members.contains_key(p))
                .collect()
        };
        let mine = within(self.mine);
        if mine.is_empty() {
            within(self.all)
        } else {
            mine
        }
    }
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
        mine,
        all,
        read,
    })
}
