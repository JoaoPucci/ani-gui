//! The run a walk first goes to: the anchor's own, or, for a side story
//! alone or in a run of one-offs, its earliest parent story's (which
//! entries are its parent stories: `franchise_walk_parents.rs`; what a
//! one-off is: `franchise_walk_series.rs`).

use std::collections::BTreeSet;

use super::place::place;
use super::series::is_series;
use super::walking::{walk, Run};
use super::Lookup;

/// Whether `run` holds a series (`franchise_walk_series.rs`).
fn holds_series(run: &Run, look: &Lookup<'_>) -> bool {
    run.members
        .keys()
        .filter_map(|m| look.entry(m, &run.consulted))
        .any(|e| is_series(&e))
}

/// The anchor's run, or, for a side story whose run is itself alone or
/// a run of one-offs (no series among them) that names a parent story,
/// its earliest parent story's.
///
/// # Errors
/// The listings to read before the walk can go on.
pub(super) fn first(anchor: &str, look: &Lookup<'_>) -> Result<Run, Vec<String>> {
    let own = walk(anchor, look, BTreeSet::new())?;
    if own.members.len() > 1 && holds_series(&own, look) {
        return Ok(own);
    }
    let placed = place(anchor, &own, look)?;
    let mut consulted = own.consulted.clone();
    consulted.extend(placed.read.iter().cloned());
    let Some(parent) = placed.root.clone() else {
        return Ok(Run { consulted, ..own });
    };
    let theirs = walk(&parent, look, consulted)?;
    let side_anchor = !theirs.members.contains_key(anchor);
    let parents = if side_anchor {
        placed.parents_in(&theirs.members)
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
