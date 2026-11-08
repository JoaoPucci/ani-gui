//! Groups of entries joined whenever two of them share a group: the
//! union of rule 2's side-by-side groups across the entries that list
//! them.

use std::collections::{BTreeMap, BTreeSet};

/// A union of groups of ids.
#[derive(Default)]
pub(super) struct Joined {
    parent: BTreeMap<String, String>,
}

impl Joined {
    fn root(&self, id: &str) -> String {
        let mut at = id.to_string();
        while let Some(p) = self.parent.get(&at).filter(|p| **p != at) {
            at = p.clone();
        }
        at
    }

    /// Join every id of `group` into one group.
    pub(super) fn join(&mut self, group: &[String]) {
        for id in group {
            self.parent.entry(id.clone()).or_insert_with(|| id.clone());
        }
        let Some(first) = group.first() else { return };
        for id in &group[1..] {
            let (a, b) = (self.root(first), self.root(id));
            if a != b {
                let (lo, hi) = if a < b { (a, b) } else { (b, a) };
                self.parent.insert(hi, lo);
            }
        }
    }

    /// The groups, each in id order.
    pub(super) fn groups(&self) -> Vec<BTreeSet<String>> {
        let mut by_root: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for id in self.parent.keys() {
            by_root.entry(self.root(id)).or_default().insert(id.clone());
        }
        by_root.into_values().collect()
    }
}
