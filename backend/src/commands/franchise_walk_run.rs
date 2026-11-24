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
    /// A side-story anchor's parent stories, in start order, set on the
    /// main story's run (`side_chain`).
    pub(crate) parents: Vec<String>,
    /// For an anchor placed by its run's parents
    /// (`franchise_walk_place.rs`): the parent stories it names outside
    /// its run, then those any member of its run names, each earliest
    /// first. A side story's parents are the first list's in the main
    /// story, else the first list's the main story leaves out (on the
    /// earliest one's chain), else the second list's, the same way.
    pub(crate) parent_choices: Vec<Vec<String>>,
    /// The other entries of the anchor's own run of one-offs, when the
    /// walk moved to their parent's run and that run left them out:
    /// listed around it like the anchor.
    pub(crate) beside: BTreeSet<String>,
    /// The franchise's entries (every entry joined to the main story by
    /// prequel and sequel links, `franchise_walk_root.rs`) that the main
    /// story leaves out, the anchor among them when it is one: listed
    /// around it, where its members' listings place them, else among the
    /// other versions (a movie, the films). They are
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
        parent_choices: Vec::new(),
        beside: BTreeSet::new(),
        moved: BTreeSet::new(),
        chain: None,
    })
}

/// A side-story anchor's parents and the chain it is drawn on, from its
/// own parent stories before its run's (`Run::parent_choices`): those
/// in the main story and no chain; else, where the main story leaves
/// them all out, those on the chain of the earliest, drawn on it,
/// or on that parent alone where it is chained to nothing (its only
/// links being to versions or settings of it). Any
/// other anchor has no parents, and its own chain.
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
    // The anchor's own parents before its run's: those in the main
    // story, else those the main story leaves out, drawn on the
    // earliest one's chain (or on it alone, chained to nothing).
    for choice in &first.parent_choices {
        let in_main: Vec<String> = choice
            .iter()
            .filter(|p| main.members.contains_key(*p))
            .cloned()
            .collect();
        if !in_main.is_empty() {
            return (in_main, None);
        }
        let in_franchise: Vec<String> = choice
            .iter()
            .filter(|p| franchise.contains(*p))
            .cloned()
            .collect();
        if let Some(p) = in_franchise.first() {
            let drawn = chain(p, franchise, main, look).unwrap_or_else(|| ChainIds {
                own: BTreeSet::from([p.clone()]),
                seams: BTreeSet::new(),
            });
            // Only the parents on the chain it is drawn on: another of
            // them may sit on a chain of its own.
            let on_chain = in_franchise
                .into_iter()
                .filter(|q| drawn.own.contains(q) || drawn.seams.contains(q))
                .collect();
            return (on_chain, Some(drawn));
        }
    }
    (Vec::new(), None)
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
    let outside = |m: &&String| !settled.members.contains_key(*m);
    let moved = franchise.iter().filter(outside).cloned().collect();
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
