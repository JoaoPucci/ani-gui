//! The run as one list: release order, or the links' order with each
//! cycle walked along its links.

use std::collections::{BTreeMap, BTreeSet};

use super::by_start::by_start;
use super::graph::{link_graph, RunGraph};
use super::place::released_line;
use super::{WatchEntry, WatchOrder, WatchRun};

/// A cycle's members in the order its links give: from the earliest
/// member a link enters by, follow each member's earliest unvisited
/// sequel inside the cycle; where that runs out, carry on from the
/// earliest member not yet listed.
fn cycle_walk<'a>(graph: &RunGraph<'a>, by_date: &[&'a WatchEntry]) -> Vec<&'a WatchEntry> {
    let comp = graph.component.get(&by_date[0].id);
    let inside = |e: &WatchEntry| graph.component.get(&e.id) == comp;
    let entered = by_date
        .iter()
        .find(|e| graph.before(&e.id).iter().any(|p| !inside(p)));
    let mut out: Vec<&WatchEntry> = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut cur = Some(*entered.unwrap_or(&by_date[0]));
    while out.len() < by_date.len() {
        let here = cur.unwrap_or_else(|| {
            *by_date
                .iter()
                .find(|e| !seen.contains(e.id.as_str()))
                .expect("a member left")
        });
        out.push(here);
        seen.insert(&here.id);
        cur = graph
            .after(&here.id)
            .iter()
            .find(|e| inside(e) && !seen.contains(e.id.as_str()))
            .copied();
    }
    out
}

/// The run as one list for a rail or the franchise page: release order,
/// or the links' order — components in topological order, the earliest
/// start first among those ready, a cycle's members along its links.
#[must_use]
pub fn sequence(run: &WatchRun, order: WatchOrder) -> Vec<&WatchEntry> {
    let line = released_line(run);
    if order == WatchOrder::Released {
        return line;
    }
    let graph = link_graph(run);
    // Components in the order their first member appears by date.
    let mut keys: Vec<usize> = Vec::new();
    let mut members: BTreeMap<usize, Vec<&WatchEntry>> = BTreeMap::new();
    for e in &line {
        let c = graph.component[&e.id];
        if !members.contains_key(&c) {
            keys.push(c);
        }
        members.entry(c).or_default().push(e);
    }
    for list in members.values_mut() {
        if list.len() > 1 {
            *list = cycle_walk(&graph, list);
        }
    }
    let mut indegree: BTreeMap<usize, usize> = keys.iter().map(|&c| (c, 0)).collect();
    let mut edges: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (id, after) in &graph.next {
        let ca = graph.component[id];
        for e in after {
            let cb = graph.component[&e.id];
            let out = edges.entry(ca).or_default();
            if ca == cb || out.contains(&cb) {
                continue;
            }
            out.push(cb);
            *indegree.get_mut(&cb).expect("a component") += 1;
        }
    }
    let first = |c: &usize| members[c][0];
    let mut ready: Vec<usize> = keys.iter().copied().filter(|c| indegree[c] == 0).collect();
    let mut result: Vec<&WatchEntry> = Vec::new();
    while !ready.is_empty() {
        ready.sort_by(|a, b| by_start(first(a), first(b)));
        let c = ready.remove(0);
        result.extend(&members[&c]);
        for d in edges.get(&c).into_iter().flatten() {
            let n = indegree.get_mut(d).expect("a component");
            *n -= 1;
            if *n == 0 {
                ready.push(*d);
            }
        }
    }
    result
}
