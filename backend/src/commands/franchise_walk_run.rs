//! The walk's driver: a run from its root, and the anchor's run, from
//! its earliest parent story when it is a side story alone.
//!
//! Kitsu sometimes lists a parent and its side story the wrong way
//! round, from both sides: One Piece names LEGO ONE PIECE, its
//! two-episode spin-off, as its parent story, and LEGO names One Piece
//! as its side story. So an entry is taken as the anchor's parent story
//! only when it is at least as much a hub as the anchor: its listing
//! names at least as many side stories, spin-offs and recaps. And an
//! entry the anchor names as its side story or spin-off is taken as its
//! parent story instead when that entry started before the anchor and
//! names more of them.

use std::collections::{BTreeMap, BTreeSet};

use super::alt::listed;
use super::rules::Walk;
use super::{order_key, Lookup};
use crate::meta::kitsu::FranchiseRelation;

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

/// Roles under which a listing names an entry beneath it.
const BENEATH: [&str; 3] = ["side_story", "spinoff", "summary"];

/// How many entries `id`'s listing names beneath it, when read.
fn beneath(look: &Lookup<'_>, id: &str) -> Option<usize> {
    look.listing(id)?;
    let ids: BTreeSet<&str> = listed(look, id, &BENEATH).map(|r| r.id.as_str()).collect();
    Some(ids.len())
}

/// The anchor's parent stories, earliest first: those it names as its
/// parent story that name at least as many entries beneath them as it
/// does, and those it names as its side story or spin-off that started
/// before it and name more.
///
/// # Errors
/// The candidates' listings to read first.
fn parent_stories(anchor: &str, look: &Lookup<'_>) -> Result<Vec<String>, Vec<String>> {
    let start = |r: &FranchiseRelation| r.entry.as_ref().and_then(|e| e.start.clone());
    let own_start = look.entry(anchor, &BTreeSet::new()).and_then(|e| e.start);
    let named: BTreeMap<&str, Option<String>> = listed(look, anchor, &["parent_story"])
        .map(|r| (r.id.as_str(), start(r)))
        .collect();
    let earlier: BTreeMap<&str, Option<String>> = listed(look, anchor, &["side_story", "spinoff"])
        .filter(|r| matches!((start(r), &own_start), (Some(a), Some(b)) if a < *b))
        .map(|r| (r.id.as_str(), start(r)))
        .collect();
    let unread: Vec<String> = named
        .keys()
        .chain(earlier.keys())
        .filter(|id| look.listing(id).is_none())
        .map(|id| (*id).to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !unread.is_empty() {
        return Err(unread);
    }
    let own = beneath(look, anchor).unwrap_or(0);
    let size = |id: &str| beneath(look, id).unwrap_or(0);
    let mut parents: Vec<(&str, Option<String>)> = named
        .iter()
        .filter(|(id, _)| size(id) >= own)
        .chain(earlier.iter().filter(|(id, _)| size(id) > own))
        .map(|(id, s)| (*id, s.clone()))
        .collect();
    parents.sort_by_key(|(id, s)| order_key(s.as_deref(), id));
    parents.dedup_by_key(|(id, _)| *id);
    Ok(parents.into_iter().map(|(id, _)| id.to_string()).collect())
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
    let parents = parent_stories(anchor, look)?;
    let Some(parent) = parents.first() else {
        return Ok(own);
    };
    let theirs = walk(parent, look, own.consulted.clone())?;
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
