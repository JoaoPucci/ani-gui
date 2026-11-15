//! The franchise walk, as a pure function over the relation listings
//! read so far.
//!
//! [`plan`] either names the listings (and entry details) it must read
//! before it can decide, or returns the decided [`Franchise`]. The IO
//! layer (`commands::franchise`) reads what it names and asks again;
//! each round starts from scratch, so the answer depends only on what
//! the listings say, never on the order they arrived in.
//!
//! The run follows the rules `docs/title-resolution.md` documents under
//! "Kitsu's relations, walked into a franchise" ([`rules`]), walked
//! from the franchise's root rather than from the anchor, so every page of a franchise answers with one main
//! story ([`root`]); the strips around it are attached from the run
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
    /// The franchise's name (`franchise_walk_name.rs`).
    pub name: String,
    /// The main-story entry the name comes from, whose artwork the
    /// franchise page and the front of the strip's poster stack show.
    pub name_from: String,
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
    /// The page's own chain, when the main story leaves the page out, or
    /// the chain a side story whose parents it leaves out is drawn on
    /// (`franchise_walk_chain.rs`); absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<Chain>,
}

/// A page's own chain, outside the main story.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chain {
    /// The chain's own titles and its seams, start order (unknown last),
    /// then numeric id.
    pub entries: Vec<FranchiseEntry>,
    /// Prequel → sequel pairs between two of its entries.
    pub links: Vec<(String, String)>,
    /// The entries that are main-story titles, where the chain leaves
    /// the main story or leads into it; the rest are its own.
    pub seams: Vec<String>,
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
    /// The franchise, boxed: it is many times the size of a need.
    Done(Box<Franchise>),
}

/// The walk from `anchor` over the `known` listings and `details`.
#[must_use]
pub fn plan(
    anchor: &str,
    known: &BTreeMap<String, FranchiseListing>,
    details: &BTreeMap<String, FranchiseEntry>,
) -> Plan {
    // The anchor's detail, then its listing, each listing read without
    // its music videos (`franchise_walk_music.rs`).
    let read = match music::read_for(anchor, known, details) {
        Ok(read) => read,
        Err((listings, details)) => return Plan::Need { listings, details },
    };
    let lookup = Lookup {
        known: &read,
        details,
    };
    match rules::run(anchor, &lookup) {
        Err(listings) => Plan::Need {
            listings,
            details: Vec::new(),
        },
        Ok(run) => Plan::Done(Box::new(around::assemble(anchor, &run, &lookup))),
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
#[path = "franchise_walk_candidates.rs"]
mod candidates;
#[path = "franchise_walk_chain.rs"]
mod chain;
#[path = "franchise_walk_chain_served.rs"]
mod chain_served;
#[path = "franchise_walk_first.rs"]
mod first;
#[path = "franchise_walk_graph.rs"]
mod graph;
#[path = "franchise_walk_joined.rs"]
mod joined;
#[path = "franchise_walk_links.rs"]
mod links;
#[path = "franchise_walk_meet.rs"]
mod meet;
#[path = "franchise_walk_music.rs"]
pub(crate) mod music;
#[path = "franchise_walk_name.rs"]
mod name;
#[path = "franchise_walk_parents.rs"]
mod parents;
#[path = "franchise_walk_place.rs"]
mod place;
#[path = "franchise_walk_prequels.rs"]
mod prequels;
#[path = "franchise_walk_root.rs"]
mod root;
#[path = "franchise_walk_rules.rs"]
pub(crate) mod rules;
#[path = "franchise_walk_sections.rs"]
mod sections;
#[path = "franchise_walk_series.rs"]
pub(crate) mod series;
#[path = "franchise_walk_run.rs"]
mod walking;

#[cfg(test)]
#[path = "franchise_walk_test.rs"]
mod tests;

#[cfg(test)]
#[path = "franchise_walk_prop_test.rs"]
mod prop_tests;

#[cfg(test)]
#[path = "franchise_corpus_test.rs"]
mod corpus_tests;

#[cfg(test)]
#[path = "franchise_corpus_record_test.rs"]
mod corpus_record;

#[cfg(test)]
#[path = "franchise_walk_parent_prop_test.rs"]
mod parent_prop_tests;

#[cfg(test)]
#[path = "franchise_walk_series_test.rs"]
mod series_tests;

#[cfg(test)]
#[path = "franchise_walk_name_test.rs"]
mod name_tests;

#[cfg(test)]
#[path = "franchise_walk_root_prop_test.rs"]
mod root_prop_tests;

#[cfg(test)]
#[path = "franchise_corpus_page_test.rs"]
mod corpus_page_tests;

#[cfg(test)]
#[path = "franchise_corpus_chain_test.rs"]
mod corpus_chain_tests;

#[cfg(test)]
#[path = "franchise_walk_chain_test.rs"]
mod chain_tests;

#[cfg(test)]
#[path = "franchise_walk_bound_test.rs"]
mod bound_tests;
