//! What sits around a run, from its members' listings, and the
//! response assembled from it:
//!
//! - **versions** (rule 4): the entries rules 2 and 3 left out, and the
//!   anime members list as `alternative_version`;
//! - **recaps**: what members list as `summary`;
//! - **films** and **sides**: the rest, prequel and sequel aside (run or
//!   versions), `character`, `adaptation` and the alternatives aside,
//!   manga and music aside; a `movie` is a film, anything else a side.
//!
//! An id lands in one section only: entries, then versions, recaps,
//! films, sides. A side-story anchor outside the run is a side.

use std::collections::BTreeSet;

use super::links::{links, parents};
use super::rules::Run;
use super::{order_key, Franchise, Lookup};
use crate::meta::kitsu::FranchiseEntry;

/// Roles that never place an entry around the run.
const NOT_AROUND: [&str; 6] = [
    "prequel",
    "sequel",
    "character",
    "adaptation",
    "alternative_setting",
    "alternative_version",
];

/// Section ids before they are told apart.
#[derive(Default)]
struct Sections {
    versions: BTreeSet<String>,
    recaps: BTreeSet<String>,
    films: BTreeSet<String>,
    sides: BTreeSet<String>,
}

fn sections(run: &Run, look: &Lookup<'_>) -> Sections {
    let mut s = Sections {
        versions: run.left_out.clone(),
        ..Sections::default()
    };
    let listings = run.members.keys().filter_map(|m| look.listing(m));
    for listing in listings {
        for r in &listing.relations {
            let music = r.subtype.as_deref() == Some("music");
            if r.kind != "anime" || music || run.members.contains_key(&r.id) {
                continue;
            }
            let to = match r.role.as_str() {
                "alternative_version" => &mut s.versions,
                "summary" => &mut s.recaps,
                role if NOT_AROUND.contains(&role) => continue,
                _ if r.subtype.as_deref() == Some("movie") => &mut s.films,
                _ => &mut s.sides,
            };
            to.insert(r.id.clone());
        }
    }
    s.versions.retain(|id| !run.members.contains_key(id));
    s.recaps.retain(|id| !s.versions.contains(id));
    s.films
        .retain(|id| !s.versions.contains(id) && !s.recaps.contains(id));
    s.sides
        .retain(|id| !s.versions.contains(id) && !s.recaps.contains(id) && !s.films.contains(id));
    s
}

/// The entries of `ids` in start order. Every id the walk names was
/// met through a relation of a listing it read, or is the anchor, whose
/// details it read first, so each has an entry.
fn entries(ids: &BTreeSet<String>, run: &Run, look: &Lookup<'_>) -> Vec<FranchiseEntry> {
    let mut found: Vec<FranchiseEntry> = ids
        .iter()
        .filter_map(|id| look.entry(id, &run.consulted))
        .collect();
    found.sort_by_key(|e| order_key(e.start.as_deref(), &e.id));
    found
}

/// The response for `anchor`'s `run`.
pub(super) fn assemble(anchor: &str, run: &Run, look: &Lookup<'_>) -> Franchise {
    let mut s = sections(run, look);
    if run.side_anchor {
        for set in [&mut s.versions, &mut s.recaps, &mut s.films] {
            set.remove(anchor);
        }
        s.sides.insert(anchor.to_string());
    }
    let members: BTreeSet<String> = run.members.keys().cloned().collect();
    Franchise {
        anchor: anchor.to_string(),
        entries: entries(&members, run, look),
        versions: entries(&s.versions, run, look),
        films: entries(&s.films, run, look),
        sides: entries(&s.sides, run, look),
        recaps: entries(&s.recaps, run, look),
        links: links(run, look),
        parents: parents(anchor, run, look),
        complete: run.complete
            && run
                .consulted
                .iter()
                .all(|c| look.listing(c).is_some_and(|l| l.complete)),
    }
}
