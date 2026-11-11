//! The walk's driver: a run from its root, and the anchor's run, from
//! its earliest parent story when it is a side story alone or in a run
//! of one-offs (which entries are its parent stories:
//! `franchise_walk_parents.rs`; what a one-off is:
//! `franchise_walk_series.rs`).

use std::collections::{BTreeMap, BTreeSet};

use super::parents::parent_stories;
use super::rules::Walk;
use super::series::is_series;
use super::{order_key, Lookup};

/// A decided run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Run {
    /// Each member and the entry before it on its path to the root.
    pub(crate) members: BTreeMap<String, Option<String>>,
    /// The entries rules 2 and 3 left out.
    pub(crate) left_out: BTreeSet<String>,
    /// Every listing the walk read.
    pub(crate) consulted: BTreeSet<String>,
    /// Whether the walk ran to its end within its bounds.
    pub(crate) complete: bool,
    /// Whether the run is the anchor's parent's, the anchor a side story.
    pub(crate) side_anchor: bool,
    /// A side-story anchor's parent stories in the run, in start order.
    pub(crate) parents: Vec<String>,
    /// The other entries of the anchor's own run of one-offs, when the
    /// walk moved to their parent's run and that run left them out:
    /// listed around it like the anchor, music videos aside.
    pub(crate) beside: BTreeSet<String>,
}

/// Walk from `root`, the listings in `consulted` already counted.
///
/// # Errors
/// The listings to read before the walk can go on.
pub(crate) fn walk(
    root: &str,
    look: &Lookup<'_>,
    mut consulted: BTreeSet<String>,
) -> Result<Run, Vec<String>> {
    consulted.insert(root.to_string());
    if look.listing(root).is_none() {
        return Err(vec![root.to_string()]);
    }
    let mut w = Walk {
        look,
        members: BTreeMap::from([(root.to_string(), None)]),
        left_out: BTreeSet::new(),
        consulted,
        complete: true,
    };
    let mut frontier = vec![root.to_string()];
    while !frontier.is_empty() {
        let (added, stop) = w.step(&frontier)?;
        if stop {
            w.complete = false;
            break;
        }
        frontier = added;
    }
    Ok(Run {
        members: w.members,
        left_out: w.left_out,
        consulted: w.consulted,
        complete: w.complete,
        side_anchor: false,
        parents: Vec::new(),
        beside: BTreeSet::new(),
    })
}

/// Whether `run` holds a series (`franchise_walk_series.rs`).
fn holds_series(run: &Run, look: &Lookup<'_>) -> bool {
    run.members
        .keys()
        .filter_map(|m| look.entry(m, &run.consulted))
        .any(|e| is_series(&e))
}

/// Where a side story alone, or a run of one-offs, is placed.
struct Placed {
    /// The parent story to walk from: the earliest any member names.
    root: Option<String>,
    /// The anchor's parents to name: its own, else its run's.
    parents: Vec<String>,
    /// Every candidate listing read to decide.
    read: BTreeSet<String>,
}

/// The parent stories the members of `own` name outside it (each
/// checked with its own start), the earliest the one to walk from; the
/// anchor's own are the ones it is placed under, else every member's.
fn place(anchor: &str, own: &Run, look: &Lookup<'_>) -> Result<Placed, Vec<String>> {
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

/// The anchor's run, or, for a side story whose run is itself alone or
/// a run of one-offs (no series among them) that names a parent story,
/// its earliest parent story's.
///
/// # Errors
/// The listings to read before the walk can go on.
pub(crate) fn run(anchor: &str, look: &Lookup<'_>) -> Result<Run, Vec<String>> {
    let own = walk(anchor, look, BTreeSet::new())?;
    if own.members.len() > 1 && holds_series(&own, look) {
        return Ok(own);
    }
    let placed = place(anchor, &own, look)?;
    let mut consulted = own.consulted.clone();
    consulted.extend(placed.read);
    let Some(parent) = placed.root else {
        return Ok(Run { consulted, ..own });
    };
    let theirs = walk(&parent, look, consulted)?;
    let side_anchor = !theirs.members.contains_key(anchor);
    let parents = if side_anchor {
        placed
            .parents
            .into_iter()
            .filter(|p| theirs.members.contains_key(p))
            .collect()
    } else {
        Vec::new()
    };
    let beside = own
        .members
        .keys()
        .filter(|m| m.as_str() != anchor && !theirs.members.contains_key(*m))
        .filter(|m| {
            look.entry(m, &theirs.consulted)
                .is_none_or(|e| e.subtype.as_deref() != Some("music"))
        })
        .cloned()
        .collect();
    Ok(Run {
        complete: own.complete && theirs.complete,
        side_anchor,
        parents,
        beside,
        ..theirs
    })
}
