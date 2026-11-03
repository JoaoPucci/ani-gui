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

use super::{order_key, Lookup, MAX_LISTINGS, MAX_MEMBERS};
use crate::meta::kitsu::FranchiseRelation;

/// A decided run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Run {
    /// Each member and the entry before it on its path to the root.
    pub(crate) members: BTreeMap<String, Option<String>>,
    /// The entries rules 2 and 3 left out.
    pub(crate) left_out: BTreeSet<String>,
    /// Every listing the walk read.
    pub(crate) consulted: BTreeSet<String>,
    /// Whether the walk ran to its end within its bounds.
    pub(crate) complete: bool,
    /// Whether the run is the anchor's parent's, the anchor a side story.
    pub(crate) side_anchor: bool,
}

const ALTERNATIVES: [&str; 2] = ["alternative_version", "alternative_setting"];

/// Whether `a`'s listing names `b` as an alternative version or setting.
fn lists_alt(look: &Lookup<'_>, a: &str, b: &str) -> bool {
    look.listing(a).is_some_and(|l| {
        l.relations
            .iter()
            .any(|r| r.id == b && r.kind == "anime" && ALTERNATIVES.contains(&r.role.as_str()))
    })
}

/// Whether `a` and `b` list each other, either way, as alternatives.
pub(crate) fn alt(look: &Lookup<'_>, a: &str, b: &str) -> bool {
    lists_alt(look, a, b) || lists_alt(look, b, a)
}

/// The anime `listing_of` lists under `role`, itself excepted.
pub(crate) fn listed<'a>(
    look: &'a Lookup<'_>,
    listing_of: &'a str,
    roles: &'a [&'a str],
) -> impl Iterator<Item = &'a FranchiseRelation> + 'a {
    look.listing(listing_of)
        .into_iter()
        .flat_map(|l| &l.relations)
        .filter(move |r| {
            r.kind == "anime" && r.id != listing_of && roles.contains(&r.role.as_str())
        })
}

/// An entry met at the current distance: the entry it was met through
/// and the roles that entry lists it under.
struct Met {
    via: String,
    roles: BTreeSet<String>,
}

/// One walk's state.
struct Walk<'l, 'a> {
    look: &'l Lookup<'a>,
    members: BTreeMap<String, Option<String>>,
    left_out: BTreeSet<String>,
    consulted: BTreeSet<String>,
    complete: bool,
}

impl Walk<'_, '_> {
    fn key(&self, id: &str) -> (bool, String, u64, String) {
        let entry = self.look.entry(id, &self.consulted);
        order_key(entry.as_ref().and_then(|e| e.start.as_deref()), id)
    }

    fn subtype(&self, id: &str) -> Option<String> {
        self.look.entry(id, &self.consulted).and_then(|e| e.subtype)
    }

    /// The entries `frontier` lists as prequel or sequel and nothing
    /// has decided yet, each met through the earliest of them.
    fn meet(&self, frontier: &[String]) -> BTreeMap<String, Met> {
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
    fn within_bound(&self, met: &BTreeMap<String, Met>) -> (Vec<String>, bool) {
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
    fn step(&mut self, frontier: &[String]) -> Result<(Vec<String>, bool), Vec<String>> {
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

/// The groups of `ids` joined, directly or through one another, by an
/// alternative link.
fn components(look: &Lookup<'_>, ids: &[String]) -> Vec<Vec<String>> {
    let mut left: Vec<String> = ids.to_vec();
    let mut out = Vec::new();
    while let Some(first) = left.pop() {
        let mut group = vec![first];
        let mut i = 0;
        while i < group.len() {
            let (joined, rest): (Vec<String>, Vec<String>) =
                left.into_iter().partition(|c| alt(look, c, &group[i]));
            group.extend(joined);
            left = rest;
            i += 1;
        }
        out.push(group);
    }
    out
}

/// Walk from `root`, the listings in `consulted` already counted.
///
/// # Errors
/// The listings to read before the walk can go on.
pub(crate) fn walk(
    root: &str,
    look: &Lookup<'_>,
    mut consulted: BTreeSet<String>,
) -> Result<Run, Vec<String>> {
    consulted.insert(root.to_string());
    if look.listing(root).is_none() {
        return Err(vec![root.to_string()]);
    }
    let mut w = Walk {
        look,
        members: BTreeMap::from([(root.to_string(), None)]),
        left_out: BTreeSet::new(),
        consulted,
        complete: true,
    };
    let mut frontier = vec![root.to_string()];
    while !frontier.is_empty() {
        let (added, stop) = w.step(&frontier)?;
        if stop {
            w.complete = false;
            break;
        }
        frontier = added;
    }
    Ok(Run {
        members: w.members,
        left_out: w.left_out,
        consulted: w.consulted,
        complete: w.complete,
        side_anchor: false,
    })
}

/// The anchor's run, or, for a side story whose run is itself alone,
/// its earliest parent story's.
///
/// # Errors
/// The listings to read before the walk can go on.
pub(crate) fn run(anchor: &str, look: &Lookup<'_>) -> Result<Run, Vec<String>> {
    let own = walk(anchor, look, BTreeSet::new())?;
    if own.members.len() > 1 {
        return Ok(own);
    }
    let parent = listed(look, anchor, &["parent_story"])
        .min_by_key(|r| order_key(r.entry.as_ref().and_then(|e| e.start.as_deref()), &r.id));
    let Some(parent) = parent else {
        return Ok(own);
    };
    let theirs = walk(&parent.id, look, own.consulted.clone())?;
    Ok(Run {
        complete: own.complete && theirs.complete,
        side_anchor: !theirs.members.contains_key(anchor),
        ..theirs
    })
}
