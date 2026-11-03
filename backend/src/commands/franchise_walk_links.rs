//! The run's own links, and a side-story anchor's parents in it.

use std::collections::{BTreeMap, BTreeSet};

use super::rules::{listed, Run};
use super::{id_key, order_key, Lookup};

/// Every prequel → sequel pair between two members, from either side.
pub(super) fn links(run: &Run, look: &Lookup<'_>) -> Vec<(String, String)> {
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

/// The side-story anchor's parents in the run, in start order.
pub(super) fn parents(anchor: &str, run: &Run, look: &Lookup<'_>) -> BTreeMap<String, Vec<String>> {
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
