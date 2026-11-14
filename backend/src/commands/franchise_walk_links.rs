//! The run's own links, and a side-story anchor's parents in it.

use std::collections::{BTreeMap, BTreeSet};

use super::rules::{listed, Run};
use super::{id_key, Lookup};

/// Every prequel → sequel pair between two members, from either side.
pub(super) fn links(run: &Run, look: &Lookup<'_>) -> Vec<(String, String)> {
    let members: BTreeSet<String> = run.members.keys().cloned().collect();
    links_among(&members, look)
}

/// Every prequel → sequel pair between two of `ids`, from either side.
pub(super) fn links_among(ids: &BTreeSet<String>, look: &Lookup<'_>) -> Vec<(String, String)> {
    let mut out = BTreeSet::new();
    for m in ids {
        for r in listed(look, m, &["prequel", "sequel"]) {
            if !ids.contains(&r.id) {
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

/// The side-story anchor's parents in the run, in start order; none
/// when the main story holds none of them.
pub(super) fn parents(anchor: &str, run: &Run) -> BTreeMap<String, Vec<String>> {
    if !run.side_anchor || run.parents.is_empty() {
        return BTreeMap::new();
    }
    BTreeMap::from([(anchor.to_string(), run.parents.clone())])
}
