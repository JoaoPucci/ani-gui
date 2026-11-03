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

use std::collections::BTreeMap;

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
    let _ = (known, details);
    Plan::Done(Franchise {
        anchor: anchor.to_string(),
        entries: Vec::new(),
        links: Vec::new(),
        versions: Vec::new(),
        films: Vec::new(),
        sides: Vec::new(),
        recaps: Vec::new(),
        parents: BTreeMap::new(),
        complete: false,
    })
}
