//! The record a run keeps of a show's stamp repairs, and the decision
//! each repair takes against it: split from [`super`] for the per-file
//! complexity bar.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::rule::{outstanding, standing, Carrier, Repairs, Stamps};
use crate::commands::listing_tags::Hearing;

/// The repairs a show's stamps took this run: the stamps from before
/// the run's first repair, each repair that still has a say, with its
/// hearing and what its listing made of those stamps ([`standing`]),
/// and the stamps they left. Stamps that are no longer the ones they
/// left were written by something else since — a removal, another
/// instance — and a repair starts over from them.
pub(super) struct Taken {
    pub(super) base: Stamps,
    pub(super) repairs: Repairs,
    pub(super) left: Stamps,
    /// Every listing at hand this run, repairing or not: a former tag
    /// any of them carries is not kept, whatever order they came in.
    pub(super) carriers: Vec<Carrier>,
}

#[cfg(test)]
impl Taken {
    /// One repair heard as `heard`, from `base`, that left `left`.
    pub(super) fn one(heard: Hearing, base: Stamps, left: Stamps) -> Self {
        Self {
            base,
            repairs: vec![(heard, left.clone())],
            left,
            carriers: Vec::new(),
        }
    }
}

/// The stamps a repair from a listing carrying `carrier`, heard as
/// `heard`, leaves over `current`, the show's stamps now, noting it in
/// `taken` under `key`. `ours` is what its listing makes of the stamps
/// it starts from. A record whose stamps were changed since starts
/// over. A former tag any listing at hand this run carries is not kept;
/// a repair that knows no hearing drops what its own listing carries.
pub(super) fn settled(
    taken: &mut BTreeMap<(PathBuf, String), Taken>,
    key: (PathBuf, String),
    current: &Stamps,
    heard: Option<Hearing>,
    carrier: &Carrier,
    ours: impl FnOnce(&Stamps) -> Stamps,
) -> Stamps {
    if taken.get(&key).is_some_and(|t| t.left != *current) {
        taken.remove(&key);
    }
    let mut next = decided(taken, key.clone(), current.clone(), heard, ours)
        .unwrap_or_else(|| current.clone());
    if heard.is_none() {
        next.former = next.former.filter(|(_, f)| !carrier.carries(f));
        return next;
    }
    let t = taken.entry(key).or_insert_with(|| Taken {
        base: current.clone(),
        repairs: Vec::new(),
        left: current.clone(),
        carriers: Vec::new(),
    });
    if !t.carriers.contains(carrier) {
        t.carriers.push(carrier.clone());
    }
    next.former = next
        .former
        .filter(|(_, f)| !t.carriers.iter().any(|c| c.carries(f)));
    t.left = next.clone();
    next
}

/// The stamps a repair heard as `heard` writes over `current`, the
/// show's stamps now, or `None` to write nothing, noting the repair in
/// `taken` under `key`. `ours` is what its listing makes of the stamps
/// it starts from (`repaired`).
pub(super) fn decided(
    taken: &mut BTreeMap<(PathBuf, String), Taken>,
    key: (PathBuf, String),
    current: Stamps,
    heard: Option<Hearing>,
    ours: impl FnOnce(&Stamps) -> Stamps,
) -> Option<Stamps> {
    current.display.as_ref()?;
    let Some(heard) = heard else {
        let ours = ours(&current);
        return (ours != current).then_some(ours);
    };
    let prior = taken.get(&key).filter(|t| t.left == current);
    let base = prior.map_or_else(|| current.clone(), |t| t.base.clone());
    let ours = ours(&base);
    // A listing that repairs nothing takes no part in the order: the
    // stamps are left to the listings that do, heard before it or after.
    if ours == base {
        return None;
    }
    let mut repairs = prior.map_or_else(Vec::new, |t| t.repairs.clone());
    let prior_carriers = prior.map_or_else(Vec::new, |t| t.carriers.clone());
    repairs.push((heard, ours));
    let repairs = outstanding(&repairs);
    let next = standing(&base, &repairs);
    taken.insert(
        key,
        Taken {
            base,
            repairs,
            left: next.clone(),
            carriers: prior_carriers,
        },
    );
    (next != current).then_some(next)
}
