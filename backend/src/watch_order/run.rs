//! The franchise payload as a run: the members once each, links and
//! parents kept only where they name members, and the entry the run is
//! numbered from.

use std::collections::{BTreeMap, BTreeSet};

use super::by_start::sort_by_start;
use super::{Franchise, RunLink, WatchEntry, WatchRun};

/// The run's first entry, the one its positions are counted from: among
/// the members no prequel link reaches, the earliest start, then the
/// lowest id; where every member has a prequel (the run is one loop),
/// the earliest member. None for an empty run.
#[must_use]
pub fn name_id(entries: &[WatchEntry], links: &[RunLink]) -> Option<String> {
    let reached: BTreeSet<&str> = links
        .iter()
        .filter(|(a, b)| a != b)
        .map(|(_, b)| b.as_str())
        .collect();
    let roots: Vec<&WatchEntry> = entries
        .iter()
        .filter(|e| !reached.contains(e.id.as_str()))
        .collect();
    let mut pool = if roots.is_empty() {
        entries.iter().collect()
    } else {
        roots
    };
    sort_by_start(&mut pool);
    pool.first().map(|e| e.id.clone())
}

/// `entries` with each id once, the first listing kept.
fn unique_entries(entries: &[WatchEntry]) -> Vec<WatchEntry> {
    let mut seen = BTreeSet::new();
    entries
        .iter()
        .filter(|e| seen.insert(e.id.clone()))
        .cloned()
        .collect()
}

/// Links between two different members, each pair once.
fn member_links(links: &[RunLink], members: &BTreeSet<String>) -> Vec<RunLink> {
    let mut seen = BTreeSet::new();
    links
        .iter()
        .filter(|(a, b)| {
            let fresh = seen.insert((a.clone(), b.clone()));
            fresh && a != b && members.contains(a) && members.contains(b)
        })
        .cloned()
        .collect()
}

/// Each side story's parents among the members, dropping a side story
/// left with none. A member of the run is placed in it, never as a side
/// story, so parents listed for a member are dropped.
fn member_parents(
    parents: &BTreeMap<String, Vec<String>>,
    members: &BTreeSet<String>,
) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for (side, list) in parents {
        if members.contains(side) {
            continue;
        }
        let mut seen = BTreeSet::new();
        let kept: Vec<String> = list
            .iter()
            .filter(|p| seen.insert((*p).clone()) && members.contains(*p))
            .cloned()
            .collect();
        if !kept.is_empty() {
            out.insert(side.clone(), kept);
        }
    }
    out
}

/// The payload as a run. None for an empty run.
#[must_use]
pub fn run_of(franchise: &Franchise) -> Option<WatchRun> {
    let entries = unique_entries(&franchise.entries);
    let members: BTreeSet<String> = entries.iter().map(|e| e.id.clone()).collect();
    let links = member_links(&franchise.links, &members);
    let first = name_id(&entries, &links)?;
    Some(WatchRun {
        name_id: first,
        entries,
        links,
        versions: franchise.versions.clone(),
        films: franchise.films.clone(),
        sides: franchise.sides.clone(),
        recaps: franchise.recaps.clone(),
        parents: member_parents(&franchise.parents, &members),
    })
}

/// Whether `id`'s strip would show anything: a run beyond `id` itself,
/// or parent stories.
#[must_use]
pub fn has_strip(run: &WatchRun, id: &str) -> bool {
    if run.parents.contains_key(id) {
        return true;
    }
    run.entries.len() > 1 && run.entries.iter().any(|e| e.id == id)
}
