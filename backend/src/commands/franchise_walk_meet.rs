//! Rule 1's step: the entries one distance further from the root, each
//! met through the earliest-starting entry that lists it, as many as the
//! listing bound leaves room to read.

use std::collections::{BTreeMap, BTreeSet};

use super::alt::listed;
use super::rules::Walk;
use super::MAX_LISTINGS;

/// An entry met at the current distance: the entry it was met through
/// and the roles that entry lists it under.
pub(super) struct Met {
    pub(super) via: String,
    pub(super) roles: BTreeSet<String>,
}

impl Walk<'_, '_> {
    /// The entries `frontier` lists as prequel or sequel and nothing
    /// has decided yet, each met through the earliest of them.
    pub(super) fn meet(&self, frontier: &[String]) -> BTreeMap<String, Met> {
        let mut by_start = frontier.to_vec();
        by_start.sort_by_key(|f| self.key(f));
        let mut met: BTreeMap<String, Met> = BTreeMap::new();
        for f in &by_start {
            for r in listed(self.look, f, &["prequel", "sequel"]) {
                if self.members.contains_key(&r.id) || self.left_out.contains(&r.id) {
                    continue;
                }
                let m = met.entry(r.id.clone()).or_insert_with(|| Met {
                    via: f.clone(),
                    roles: BTreeSet::new(),
                });
                if &m.via == f {
                    m.roles.insert(r.role.clone());
                }
            }
        }
        met
    }

    /// The met entries in start order, as many as the listing bound
    /// leaves room to read, and whether the bound cut them.
    pub(super) fn within_bound(&self, met: &BTreeMap<String, Met>) -> (Vec<String>, bool) {
        let mut ids: Vec<String> = met.keys().cloned().collect();
        ids.sort_by_key(|id| self.key(id));
        let room = MAX_LISTINGS.saturating_sub(self.consulted.len());
        let mut fresh = 0;
        let mut kept = Vec::new();
        for id in ids {
            if !self.consulted.contains(&id) {
                if fresh == room {
                    return (kept, true);
                }
                fresh += 1;
            }
            kept.push(id);
        }
        (kept, false)
    }
}
