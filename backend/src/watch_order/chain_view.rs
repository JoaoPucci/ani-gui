//! A page outside the main story, drawn on a chain (the walk's `chain`:
//! the franchise's entries outside the main story joined to the page,
//! or, for a side story, to its parent, and the main-story titles it
//! meets, its seams). The walk decides the chain; these reshape it for
//! the strip's readings.

use std::collections::BTreeSet;

use serde::Serialize;

use super::{Franchise, WatchEntry};

/// A main-story title beside a chain's own: the one it comes from, or
/// the one it leads into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Seam {
    /// The main-story title the chain comes from.
    From,
    /// The main-story title the chain leads into.
    Into,
}

/// A column of the strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The column before the entry.
    Before,
    /// The column after it.
    After,
}

/// The franchise as the page's strip draws it: the chain's titles and
/// links as the run, and the sections without them. None on a page with
/// no chain.
#[must_use]
pub fn chain_franchise(f: &Franchise) -> Option<Franchise> {
    let chain = f.chain.as_ref()?;
    let in_chain: BTreeSet<&str> = chain.entries.iter().map(|e| e.id.as_str()).collect();
    let out = |list: &[WatchEntry]| -> Vec<WatchEntry> {
        list.iter()
            .filter(|e| !in_chain.contains(e.id.as_str()))
            .cloned()
            .collect()
    };
    Some(Franchise {
        anchor: f.anchor.clone(),
        name: f.name.clone(),
        name_from: f.name_from.clone(),
        entries: chain.entries.clone(),
        links: chain.links.clone(),
        versions: out(&f.versions),
        films: out(&f.films),
        sides: out(&f.sides),
        recaps: out(&f.recaps),
        parents: f.parents.clone(),
        complete: f.complete,
        chain: None,
    })
}

/// The chain's own titles alone, as a run: what a place on it is
/// counted over.
#[must_use]
pub fn own_franchise(f: &Franchise) -> Option<Franchise> {
    let mut drawn = chain_franchise(f)?;
    let seams: BTreeSet<&str> = f.chain.as_ref()?.seams.iter().map(String::as_str).collect();
    drawn.entries.retain(|e| !seams.contains(e.id.as_str()));
    let own: BTreeSet<String> = drawn.entries.iter().map(|e| e.id.clone()).collect();
    drawn
        .links
        .retain(|(a, b)| own.contains(a) && own.contains(b));
    Some(drawn)
}

/// The seam a column shows: a seam standing in it alone, else none.
#[must_use]
pub fn seam_of(list: &[WatchEntry], seams: &[String], side: Side) -> Option<Seam> {
    match list {
        [only] if seams.contains(&only.id) => Some(match side {
            Side::Before => Seam::From,
            Side::After => Seam::Into,
        }),
        _ => None,
    }
}
