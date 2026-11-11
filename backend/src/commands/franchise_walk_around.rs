//! What sits around a run, from its members' listings, and the
//! response assembled from it:
//!
//! - **versions** (rule 4): the entries rules 2 and 3 left out, music
//!   aside, and the anime members list as `alternative_version`;
//! - **recaps**: what members list as `summary`, a TV series aside: a
//!   series that retells another is one of its versions;
//! - **films** and **sides**: the rest, prequel and sequel aside (run or
//!   versions), `character`, `adaptation` and the alternatives aside,
//!   manga and music aside; a `movie` is a film, anything else a side.
//!
//! A movie is a film whatever the role that placed it: a version or a
//! recap that is a movie is listed under films. An id lands in one
//! section only: entries, then films (for a movie), versions, recaps,
//! sides. A side-story anchor outside the run is a side, or a film when
//! it is a movie, and so is an entry of the anchor's own run of
//! one-offs that the run it moved to leaves out.

use std::collections::BTreeSet;

use super::links::{links, parents};
use super::name::named;
use super::rules::Run;
use super::sections::{is_movie, Sections};
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

fn sections(run: &Run, look: &Lookup<'_>) -> Sections {
    let music = |id: &String| {
        look.entry(id, &run.consulted)
            .is_some_and(|e| e.subtype.as_deref() == Some("music"))
    };
    let mut s = Sections {
        versions: run
            .left_out
            .iter()
            .filter(|id| !music(id))
            .cloned()
            .collect(),
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
                // A TV series that retells another is a version of it.
                "summary" if r.subtype.as_deref() == Some("TV") => &mut s.versions,
                "summary" => &mut s.recaps,
                role if NOT_AROUND.contains(&role) => continue,
                _ if r.subtype.as_deref() == Some("movie") => &mut s.films,
                _ => &mut s.sides,
            };
            to.insert(r.id.clone());
        }
    }
    s.versions.retain(|id| !run.members.contains_key(id));
    s.movies_to_films(|id| is_movie(id, run, look));
    s.one_section_each();
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
        s.place_side_anchor(anchor, is_movie(anchor, run, look));
    }
    for id in &run.beside {
        s.place_beside(id, is_movie(id, run, look));
    }
    let members: BTreeSet<String> = run.members.keys().cloned().collect();
    let main = entries(&members, run, look);
    let [versions, films, sides, recaps] =
        [&s.versions, &s.films, &s.sides, &s.recaps].map(|ids| entries(ids, run, look));
    let (name, name_from) = named(anchor, &main, [&versions, &films, &sides, &recaps]);
    Franchise {
        anchor: anchor.to_string(),
        name,
        name_from,
        entries: main,
        versions,
        films,
        sides,
        recaps,
        links: links(run, look),
        parents: parents(anchor, run),
        complete: run.complete
            && run
                .consulted
                .iter()
                .all(|c| look.listing(c).is_some_and(|l| l.complete)),
    }
}
