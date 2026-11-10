//! The walk's driver: a run from its root, and the anchor's run, from
//! its earliest parent story when it is a side story alone (which
//! entries are its parent stories: `franchise_walk_parents.rs`).

use std::collections::{BTreeMap, BTreeSet};

use super::parents::parent_stories;
use super::rules::Walk;
use super::Lookup;

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
    })
}

/// The anchor's run, or, for a side story whose run is itself alone,
/// its earliest parent story's.
///
/// # Errors
/// The listings to read before the walk can go on.
pub(crate) fn run(anchor: &str, look: &Lookup<'_>) -> Result<Run, Vec<String>> {
    let own = walk(anchor, look, BTreeSet::new())?;
    if own.members.len() > 1 {
        return Ok(own);
    }
    let found = parent_stories(anchor, look)?;
    let mut consulted = own.consulted.clone();
    consulted.extend(found.read);
    let Some(parent) = found.parents.first() else {
        return Ok(Run { consulted, ..own });
    };
    let theirs = walk(parent, look, consulted)?;
    let parents = found.parents;
    let side_anchor = !theirs.members.contains_key(anchor);
    let parents = if side_anchor {
        parents
            .into_iter()
            .filter(|p| theirs.members.contains_key(p))
            .collect()
    } else {
        Vec::new()
    };
    Ok(Run {
        complete: own.complete && theirs.complete,
        side_anchor,
        parents,
        ..theirs
    })
}
