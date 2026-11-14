//! The walk's driver: a run from its root, and the anchor's run (the
//! one it first goes to, `franchise_walk_first.rs`) settled to the
//! franchise's main story (`franchise_walk_root.rs`).

use std::collections::{BTreeMap, BTreeSet};

use super::chain::{chain, ChainIds};
use super::first::first;
use super::root::settle;
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
    /// The other entries of the anchor's own run of one-offs, when the
    /// walk moved to their parent's run and that run left them out:
    /// listed around it like the anchor, music videos aside.
    pub(crate) beside: BTreeSet<String>,
    /// The franchise's entries (every entry joined to the main story by
    /// prequel and sequel links, `franchise_walk_root.rs`) that the main
    /// story leaves out, the anchor among them when it is one: listed
    /// around it, where its members' listings place them, else among the
    /// other versions (a movie, the films), music videos aside. They are
    /// the franchise's, so every page of it lists the same ones.
    pub(crate) moved: BTreeSet<String>,
    /// The anchor's own chain, when the main story leaves it out
    /// (`franchise_walk_chain.rs`), or, for a side story whose parents
    /// the main story leaves out, its earliest parent's chain.
    pub(crate) chain: Option<ChainIds>,
}

/// Walk from `root`, the listings in `consulted` already counted.
///
/// # Errors
/// The listings to read before the walk can go on.
pub(crate) fn walk(
    root: &str,
    look: &Lookup<'_>,
    consulted: BTreeSet<String>,
) -> Result<Run, Vec<String>> {
    walk_with(root, look, consulted, BTreeSet::new())
}

/// Walk from `root`, the listings in `consulted` already counted, the
/// links of the `reverse` listings also counted from the other side.
///
/// # Errors
/// The listings to read before the walk can go on.
pub(crate) fn walk_with(
    root: &str,
    look: &Lookup<'_>,
    mut consulted: BTreeSet<String>,
    reverse: BTreeSet<String>,
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
        reverse,
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
        moved: BTreeSet::new(),
        chain: None,
    })
}

/// A side-story anchor's parents and the chain it is drawn on: those of
/// its parents in the main story and no chain; else, where the main
/// story leaves them all out, those in the franchise, on the chain of
/// the earliest. Any other anchor has no parents, and its own chain.
fn side_chain(
    anchor: &str,
    side_anchor: bool,
    first: &Run,
    main: &Run,
    franchise: &BTreeSet<String>,
    look: &Lookup<'_>,
) -> (Vec<String>, Option<ChainIds>) {
    if !side_anchor {
        return (Vec::new(), chain(anchor, franchise, main, look));
    }
    let among = |set: &dyn Fn(&String) -> bool| -> Vec<String> {
        first.parents.iter().filter(|p| set(p)).cloned().collect()
    };
    let in_main = among(&|p| main.members.contains_key(p));
    if !in_main.is_empty() {
        return (in_main, None);
    }
    let in_franchise = among(&|p| franchise.contains(p));
    let drawn = in_franchise
        .first()
        .and_then(|p| chain(p, franchise, main, look));
    (in_franchise, drawn)
}

/// The franchise's main story (`franchise_walk_root.rs`), settled from
/// the run the walk first goes to (`franchise_walk_first.rs`); a
/// side-story anchor keeps its parents (`side_chain`).
///
/// # Errors
/// The listings to read before the walk can go on.
pub(crate) fn run(anchor: &str, look: &Lookup<'_>) -> Result<Run, Vec<String>> {
    let first = first(anchor, look)?;
    let (settled, franchise) = settle(first.clone(), look)?;
    let is_music = |m: &String| {
        look.entry(m, &settled.consulted)
            .is_some_and(|e| e.subtype.as_deref() == Some("music"))
    };
    let outside = |m: &&String| !settled.members.contains_key(*m);
    let moved = franchise
        .iter()
        .filter(outside)
        .filter(|m| m.as_str() == anchor || !is_music(m))
        .cloned()
        .collect();
    let beside = first.beside.iter().filter(outside).cloned().collect();
    let side_anchor = first.side_anchor && !settled.members.contains_key(anchor);
    let (parents, chain) = side_chain(anchor, side_anchor, &first, &settled, &franchise, look);
    Ok(Run {
        side_anchor,
        parents,
        beside,
        moved,
        chain,
        ..settled
    })
}
