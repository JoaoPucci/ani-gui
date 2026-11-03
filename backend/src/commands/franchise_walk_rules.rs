//! How the walk builds a run (rules 1–3 of the proposal's section 4):
//!
//! 1. Breadth-first from the root, one distance at a time, along the
//!    prequel and sequel links each added entry lists (anime only). An
//!    entry first met at a distance takes the path through the listing
//!    entry that started earliest, then the lowest id, and is decided
//!    once, on that path.
//! 2. Of the sequels (or prequels) one entry lists, those that list one
//!    another as `alternative_version`/`alternative_setting` keep one:
//!    the one in the listing entry's format, else the earliest, else the
//!    lowest id.
//! 3. An entry that, with any entry on its path, lists the other as an
//!    alternative version or setting is left out. The root never is.
//!
//! A side-story anchor whose run is itself alone walks from its
//! earliest parent story instead.

use std::collections::{BTreeMap, BTreeSet};

use super::alt::components;
use super::meet::Met;
use super::{order_key, Lookup, MAX_MEMBERS};

pub(crate) use super::alt::{alt, listed};
pub(crate) use super::walking::{run, Run};

/// One walk's state.
pub(super) struct Walk<'l, 'a> {
    pub(super) look: &'l Lookup<'a>,
    pub(super) members: BTreeMap<String, Option<String>>,
    pub(super) left_out: BTreeSet<String>,
    pub(super) consulted: BTreeSet<String>,
    pub(super) complete: bool,
}

impl Walk<'_, '_> {
    pub(super) fn key(&self, id: &str) -> (bool, String, u64, String) {
        let entry = self.look.entry(id, &self.consulted);
        order_key(entry.as_ref().and_then(|e| e.start.as_deref()), id)
    }

    pub(super) fn subtype(&self, id: &str) -> Option<String> {
        self.look.entry(id, &self.consulted).and_then(|e| e.subtype)
    }

    /// Rule 2: the entries a group of alternatives listed side by side
    /// leaves out.
    fn rule_two(&self, kept: &[String], met: &BTreeMap<String, Met>) -> BTreeSet<String> {
        let mut groups: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for id in kept {
            for role in &met[id].roles {
                groups
                    .entry((met[id].via.clone(), role.clone()))
                    .or_default()
                    .push(id.clone());
            }
        }
        let mut out = BTreeSet::new();
        for ((via, _), ids) in groups {
            let format = self.subtype(&via);
            for component in components(self.look, &ids) {
                let winner = component.iter().min_by_key(|c| {
                    let same = format.is_some() && self.subtype(c) == format;
                    (!same, self.key(c))
                });
                out.extend(component.iter().filter(|c| Some(*c) != winner).cloned());
            }
        }
        out
    }

    /// Rule 3: whether `id`, met through `via`, is an alternative of an
    /// entry on its path.
    fn along_path(&self, id: &str, via: &str) -> bool {
        let mut at = Some(via.to_string());
        while let Some(p) = at {
            if alt(self.look, id, &p) {
                return true;
            }
            at = self.members.get(&p).cloned().flatten();
        }
        false
    }

    /// Decide one distance. `Err` names listings to read first; `Ok`
    /// the entries added, and whether the walk must stop.
    pub(super) fn step(&mut self, frontier: &[String]) -> Result<(Vec<String>, bool), Vec<String>> {
        let met = self.meet(frontier);
        let (kept, cut) = self.within_bound(&met);
        self.consulted.extend(kept.iter().cloned());
        let need: Vec<String> = kept
            .iter()
            .filter(|id| self.look.listing(id).is_none())
            .cloned()
            .collect();
        if !need.is_empty() {
            return Err(need);
        }
        let losers = self.rule_two(&kept, &met);
        let mut added = Vec::new();
        for id in kept {
            let via = &met[&id].via;
            if losers.contains(&id) || self.along_path(&id, via) {
                self.left_out.insert(id);
                continue;
            }
            if self.members.len() >= MAX_MEMBERS {
                return Ok((added, true));
            }
            self.members.insert(id.clone(), Some(via.clone()));
            added.push(id);
        }
        Ok((added, cut))
    }
}
