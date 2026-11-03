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

use std::collections::{BTreeMap, BTreeSet};

use super::rules::{listed, Run};
use super::{id_key, order_key, Franchise, Lookup, Plan};
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
    for m in run.members.keys() {
        let Some(listing) = look.listing(m) else {
            continue;
        };
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

/// Every prequel → sequel pair between two members, from either side.
fn links(run: &Run, look: &Lookup<'_>) -> Vec<(String, String)> {
    let mut out = BTreeSet::new();
    for m in run.members.keys() {
        for r in listed(look, m, &["prequel", "sequel"]) {
            if !run.members.contains_key(&r.id) {
                continue;
            }
            out.insert(if r.role == "sequel" {
                (m.clone(), r.id.clone())
            } else {
                (r.id.clone(), m.clone())
            });
        }
    }
    let mut out: Vec<_> = out.into_iter().collect();
    out.sort_by_key(|(a, b)| (id_key(a), id_key(b)));
    out
}

/// The entries of `ids` in start order, or the ids nothing describes.
fn entries(
    ids: &BTreeSet<String>,
    run: &Run,
    look: &Lookup<'_>,
) -> Result<Vec<FranchiseEntry>, Vec<String>> {
    let mut found = Vec::new();
    let mut missing = Vec::new();
    for id in ids {
        match look.entry(id, &run.consulted) {
            Some(e) => found.push(FranchiseEntry {
                id: id.clone(),
                ..e
            }),
            None => missing.push(id.clone()),
        }
    }
    if !missing.is_empty() {
        return Err(missing);
    }
    found.sort_by_key(|e| order_key(e.start.as_deref(), &e.id));
    Ok(found)
}

/// The side-story anchor's parents in the run, in start order.
fn parents(anchor: &str, run: &Run, look: &Lookup<'_>) -> BTreeMap<String, Vec<String>> {
    if !run.side_anchor {
        return BTreeMap::new();
    }
    let mut ids: Vec<String> = listed(look, anchor, &["parent_story"])
        .filter(|r| run.members.contains_key(&r.id))
        .map(|r| r.id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    ids.sort_by_key(|id| {
        let e = look.entry(id, &run.consulted);
        order_key(e.as_ref().and_then(|e| e.start.as_deref()), id)
    });
    BTreeMap::from([(anchor.to_string(), ids)])
}

/// The response for `anchor`'s `run`, or the details still needed.
pub(super) fn assemble(anchor: &str, run: &Run, look: &Lookup<'_>) -> Plan {
    let mut s = sections(run, look);
    if run.side_anchor {
        for set in [&mut s.versions, &mut s.recaps, &mut s.films] {
            set.remove(anchor);
        }
        s.sides.insert(anchor.to_string());
    }
    let members: BTreeSet<String> = run.members.keys().cloned().collect();
    let mut missing = Vec::new();
    let mut take = |ids: &BTreeSet<String>| {
        entries(ids, run, look).unwrap_or_else(|m| {
            missing.extend(m);
            Vec::new()
        })
    };
    let franchise = Franchise {
        anchor: anchor.to_string(),
        entries: take(&members),
        versions: take(&s.versions),
        films: take(&s.films),
        sides: take(&s.sides),
        recaps: take(&s.recaps),
        links: links(run, look),
        parents: parents(anchor, run, look),
        complete: run.complete
            && run
                .consulted
                .iter()
                .all(|c| look.listing(c).is_some_and(|l| l.complete)),
    };
    if missing.is_empty() {
        Plan::Done(franchise)
    } else {
        Plan::Need {
            listings: Vec::new(),
            details: missing,
        }
    }
}
