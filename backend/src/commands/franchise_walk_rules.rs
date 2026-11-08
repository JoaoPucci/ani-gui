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
//!    lowest id. A group is decided once per distance, over every entry
//!    that lists its members, so it keeps one even when they were met
//!    through different entries, and adds none when one of them is
//!    already in the run.
//! 3. An entry that, with any entry on its path, lists the other as an
//!    alternative version or setting is left out. The root never is.
//!
//! A side-story anchor whose run is itself alone walks from its
//! earliest parent story instead.

use std::collections::{BTreeMap, BTreeSet};

use super::alt::components;
use super::joined::Joined;
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

    /// Rule 2: the entries met this step that a group of alternatives
    /// listed side by side leaves out. A group is what one frontier
    /// entry lists in one role, among the entries met this step and the
    /// run's members, joined by alternative links; groups that share an
    /// entry are one group, decided once. A group holding a member keeps
    /// that member and adds none; otherwise it adds one, the one in the
    /// format of the entry it was met through, else the earliest, else
    /// the lowest id.
    fn rule_two(
        &self,
        frontier: &[String],
        kept: &[String],
        met: &BTreeMap<String, Met>,
    ) -> BTreeSet<String> {
        let kept_set: BTreeSet<&str> = kept.iter().map(String::as_str).collect();
        let in_play = |id: &str| kept_set.contains(id) || self.members.contains_key(id);
        let mut joined = Joined::default();
        for f in frontier {
            for role in ["prequel", "sequel"] {
                let side_by_side: BTreeSet<String> = listed(self.look, f, &[role])
                    .map(|r| r.id.clone())
                    .filter(|id| in_play(id))
                    .collect();
                let side_by_side: Vec<String> = side_by_side.into_iter().collect();
                for component in components(self.look, &side_by_side) {
                    joined.join(&component);
                }
            }
        }
        let mut out = BTreeSet::new();
        for group in joined.groups() {
            if group.iter().any(|g| self.members.contains_key(g)) {
                out.extend(group.into_iter().filter(|g| kept_set.contains(g.as_str())));
                continue;
            }
            let winner = group
                .iter()
                .min_by_key(|c| {
                    let format = self.subtype(&met[*c].via);
                    let same = format.is_some() && self.subtype(c) == format;
                    (!same, self.key(c))
                })
                .cloned();
            out.extend(group.into_iter().filter(|g| Some(g) != winner.as_ref()));
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
        let losers = self.rule_two(frontier, &kept, &met);
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
