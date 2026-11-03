//! The franchise walk, as a pure function over the relation listings
//! read so far.
//!
//! [`plan`] either names the listings (and entry details) it must read
//! before it can decide, or returns the decided [`Franchise`]. The IO
//! layer (`commands::franchise`) reads what it names and asks again;
//! each round starts from scratch, so the answer depends only on what
//! the listings say, never on the order they arrived in.
//!
//! The run follows the rules of `docs/proposals/franchise-watch-order.md`
//! section 4 ([`rules`]); the strips around it are attached from the run
//! members' listings ([`around`]).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::meta::kitsu::{FranchiseEntry, FranchiseListing};

/// The most entries a run holds. A walk that reaches it stops adding
/// and is incomplete.
pub const MAX_MEMBERS: usize = 40;
/// The most listings one walk reads. A walk that reaches it stops and
/// is incomplete.
pub const MAX_LISTINGS: usize = 80;

/// The franchise around one anime, as `GET /api/kitsu/franchise/:id`
/// serves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Franchise {
    /// The id asked about.
    pub anchor: String,
    /// The run, start order (unknown last), then numeric id.
    pub entries: Vec<FranchiseEntry>,
    /// Prequel → sequel pairs between two run members.
    pub links: Vec<(String, String)>,
    /// Other versions of the run's entries.
    pub versions: Vec<FranchiseEntry>,
    /// Films around the run.
    pub films: Vec<FranchiseEntry>,
    /// Side stories, spin-offs and the rest around the run.
    pub sides: Vec<FranchiseEntry>,
    /// Recaps of the run's entries.
    pub recaps: Vec<FranchiseEntry>,
    /// A side-story anchor's parent stories in the run.
    pub parents: BTreeMap<String, Vec<String>>,
    /// False when the walk stopped at its bound or a listing was cut
    /// short.
    pub complete: bool,
}

/// What the walk needs next, or what it decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Read these listings and these entries' details, then ask again.
    Need {
        /// Ids whose relation listings are needed.
        listings: Vec<String>,
        /// Ids whose own details are needed.
        details: Vec<String>,
    },
    /// The franchise.
    Done(Franchise),
}

/// The walk from `anchor` over the `known` listings and `details`.
#[must_use]
pub fn plan(
    anchor: &str,
    known: &BTreeMap<String, FranchiseListing>,
    details: &BTreeMap<String, FranchiseEntry>,
) -> Plan {
    let missing = |has: bool| {
        if has {
            Vec::new()
        } else {
            vec![anchor.to_string()]
        }
    };
    let (listings, needed) = (
        missing(known.contains_key(anchor)),
        missing(details.contains_key(anchor)),
    );
    if !listings.is_empty() || !needed.is_empty() {
        return Plan::Need {
            listings,
            details: needed,
        };
    }
    let lookup = Lookup { known, details };
    match rules::run(anchor, &lookup) {
        Err(listings) => Plan::Need {
            listings,
            details: Vec::new(),
        },
        Ok(run) => Plan::Done(around::assemble(anchor, &run, &lookup)),
    }
}

/// The listings and details a walk reads from.
pub(crate) struct Lookup<'a> {
    pub(crate) known: &'a BTreeMap<String, FranchiseListing>,
    pub(crate) details: &'a BTreeMap<String, FranchiseEntry>,
}

impl Lookup<'_> {
    /// The listing of `id`, when read.
    pub(crate) fn listing(&self, id: &str) -> Option<&FranchiseListing> {
        self.known.get(id)
    }

    /// What is known of `id`: its own details, else the first
    /// `consulted` listing (in id order) that relates it.
    pub(crate) fn entry(&self, id: &str, consulted: &BTreeSet<String>) -> Option<FranchiseEntry> {
        if let Some(d) = self.details.get(id) {
            return Some(d.clone());
        }
        consulted
            .iter()
            .filter_map(|c| self.known.get(c))
            .flat_map(|l| &l.relations)
            .find(|r| r.id == id && r.entry.is_some())
            .and_then(|r| r.entry.clone())
    }
}

/// The sort key for entries: start (unknown last), then numeric id.
pub(crate) fn order_key(start: Option<&str>, id: &str) -> (bool, String, u64, String) {
    (
        start.is_none(),
        start.unwrap_or_default().to_string(),
        id.parse().unwrap_or(u64::MAX),
        id.to_string(),
    )
}

/// The key of a numeric id alone.
pub(crate) fn id_key(id: &str) -> (u64, String) {
    (id.parse().unwrap_or(u64::MAX), id.to_string())
}

#[path = "franchise_walk_alt.rs"]
mod alt;
#[path = "franchise_walk_around.rs"]
mod around;
#[path = "franchise_walk_links.rs"]
mod links;
#[path = "franchise_walk_meet.rs"]
mod meet;
#[path = "franchise_walk_rules.rs"]
pub(crate) mod rules;
#[path = "franchise_walk_run.rs"]
mod walking;

#[cfg(test)]
#[path = "franchise_walk_test.rs"]
mod tests;

#[cfg(test)]
#[path = "franchise_walk_prop_test.rs"]
mod prop_tests;
