//! The run's link graph: prequel/sequel links between members, with
//! shortcut links (ones a longer path already covers) dropped, and the
//! members that form a cycle.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::by_start::sort_by_start;
use super::components::components;
use super::{WatchEntry, WatchRun};

/// The run's kept links and its cycles.
#[derive(Debug, Clone)]
pub struct RunGraph<'a> {
    /// Each member's kept prequels, start order.
    pub prev: BTreeMap<String, Vec<&'a WatchEntry>>,
    /// Each member's kept sequels, start order.
    pub next: BTreeMap<String, Vec<&'a WatchEntry>>,
    /// Members that lead round to themselves.
    pub cycle: BTreeSet<String>,
    /// Strongly connected component per member.
    pub component: BTreeMap<String, usize>,
}

impl<'a> RunGraph<'a> {
    /// `id`'s kept prequels (none for an id outside the run).
    #[must_use]
    pub fn before(&self, id: &str) -> &[&'a WatchEntry] {
        self.prev.get(id).map_or(&[], Vec::as_slice)
    }

    /// `id`'s kept sequels (none for an id outside the run).
    #[must_use]
    pub fn after(&self, id: &str) -> &[&'a WatchEntry] {
        self.next.get(id).map_or(&[], Vec::as_slice)
    }
}

/// Whether `to` is reachable from `from` over component links other
/// than the direct one.
fn reachable_around(comp_out: &BTreeMap<usize, BTreeSet<usize>>, from: usize, to: usize) -> bool {
    let mut seen = BTreeSet::new();
    let mut queue: VecDeque<usize> = comp_out
        .get(&from)
        .into_iter()
        .flatten()
        .copied()
        .filter(|&c| c != to)
        .collect();
    while let Some(c) = queue.pop_front() {
        if c == to {
            return true;
        }
        if seen.insert(c) {
            queue.extend(comp_out.get(&c).into_iter().flatten().copied());
        }
    }
    false
}

/// The run's link graph.
#[must_use]
pub fn link_graph(run: &WatchRun) -> RunGraph<'_> {
    let ids: Vec<String> = run.entries.iter().map(|e| e.id.clone()).collect();
    let by_id: BTreeMap<&str, &WatchEntry> =
        run.entries.iter().map(|e| (e.id.as_str(), e)).collect();
    let mut adjacency: BTreeMap<String, Vec<String>> =
        ids.iter().map(|i| (i.clone(), Vec::new())).collect();
    for (a, b) in &run.links {
        if let Some(out) = adjacency.get_mut(a) {
            out.push(b.clone());
        }
    }
    let comp = components(&ids, &adjacency);
    let mut sizes: BTreeMap<usize, usize> = BTreeMap::new();
    for &c in comp.values() {
        *sizes.entry(c).or_default() += 1;
    }
    // Links between members only: a run's links never name anyone else
    // (`run::run_of`).
    let links: Vec<(&String, &String, usize, usize)> = run
        .links
        .iter()
        .filter_map(|(a, b)| Some((a, b, *comp.get(a)?, *comp.get(b)?)))
        .collect();
    let mut comp_out: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for &(_, _, ca, cb) in &links {
        if ca != cb {
            comp_out.entry(ca).or_default().insert(cb);
        }
    }
    let mut prev: BTreeMap<String, Vec<&WatchEntry>> =
        ids.iter().map(|i| (i.clone(), Vec::new())).collect();
    let mut next = prev.clone();
    for &(a, b, ca, cb) in &links {
        if ca != cb && reachable_around(&comp_out, ca, cb) {
            continue;
        }
        if let (Some(&ea), Some(&eb)) = (by_id.get(a.as_str()), by_id.get(b.as_str())) {
            next.entry(a.clone()).or_default().push(eb);
            prev.entry(b.clone()).or_default().push(ea);
        }
    }
    for list in prev.values_mut().chain(next.values_mut()) {
        sort_by_start(list);
    }
    let cycle = ids
        .iter()
        .filter(|i| sizes[&comp[*i]] > 1)
        .cloned()
        .collect();
    RunGraph {
        prev,
        next,
        cycle,
        component: comp,
    }
}
