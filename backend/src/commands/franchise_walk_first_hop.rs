//! The walk's first hop: what the show page can tell from the anchor's
//! own relation listing, the first listing the walk reads, before the
//! walk has read anything else.
//!
//! The show page reserves room for its watch-order strip (or its lone
//! franchise link) while the walk runs, so the strip does not push the
//! page down when it lands. [`first_hop`] says which of the two to
//! reserve, or that nothing is coming. It answers the link or nothing
//! only where it is certain the walk will, and says the strip
//! everywhere else:
//!
//! - **strip**: the show lists an anime prequel or sequel other than
//!   itself, whatever else it lists it as; else a parent story the
//!   walk's own parent check accepts (`franchise_walk_parents.rs`),
//!   whose run a side story alone takes, or a listing that check reads
//!   naming the show as its prequel or sequel: for that it reads the
//!   show's detail and the listings of the candidates the walk's parent
//!   check reads, within the walk's bound, as the walk does;
//! - **link**: else, any anime relation the franchise page lists beside
//!   the show: anything but a music video, a character crossover, an
//!   adaptation or an alternative setting;
//! - **none**: else.
//!
//! Only the strip can be wrong. Where a prequel or sequel turns out to
//! be left out of the run as another version or setting (rule 3),
//! from either side's listing, or a parent story is no parent the walk
//! walks from, the walk draws the link or nothing, and the page's
//! reserved strip gives way to it. Where the first hop says link or
//! nothing, the walk agrees.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::parents::parent_stories;
use super::Lookup;
use crate::meta::kitsu::{FranchiseEntry, FranchiseListing, FranchiseRelation};

/// What the show page draws once the walk answers, as far as the
/// anchor's own listing tells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FirstHop {
    /// The watch-order strip.
    Strip,
    /// The franchise link on its own.
    Link,
    /// Neither.
    None,
}

/// What the first hop needs next, or what it foretells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Read these listings and these entries' details, then ask again.
    Read {
        /// Ids whose relation listings are needed.
        listings: Vec<String>,
        /// Ids whose own details are needed.
        details: Vec<String>,
    },
    /// What the show page will draw.
    Done(FirstHop),
}

/// What the listings and details read so far say the show page will
/// draw for `anchor`, or what to read first.
#[must_use]
pub fn first_hop(
    anchor: &str,
    known: &BTreeMap<String, FranchiseListing>,
    details: &BTreeMap<String, FranchiseEntry>,
) -> Step {
    let Some(listing) = known.get(anchor) else {
        return read(vec![anchor.to_string()], Vec::new());
    };
    let hop = from_own_listing(anchor, listing);
    if hop == FirstHop::Strip || !names_a_possible_parent(anchor, listing) {
        return Step::Done(hop);
    }
    // No run beside the show: the walk's parent check decides whether
    // it walks from a parent story, from the show's start and the
    // candidates' own listings, as far as the walk's bound leaves room
    // beside the show's own listing.
    if !details.contains_key(anchor) {
        return read(Vec::new(), vec![anchor.to_string()]);
    }
    let counted = BTreeSet::from([anchor.to_string()]);
    match parent_stories(
        anchor,
        &Lookup { known, details },
        &BTreeSet::new(),
        &counted,
    ) {
        Err(listings) => read(listings, Vec::new()),
        Ok(found) if !found.parents.is_empty() => Step::Done(FirstHop::Strip),
        Ok(_) if named_from_the_other_side(anchor, known) => Step::Done(FirstHop::Strip),
        Ok(_) => Step::Done(hop),
    }
}

fn read(listings: Vec<String>, details: Vec<String>) -> Step {
    Step::Read { listings, details }
}

/// Whether a listing read for the parent check names `anchor` as its
/// prequel or sequel: the franchise counts a link from either side, so
/// the walk may join the two. Whether it does turns on whether either
/// side names the other as its version or setting too (rule 3) — but
/// the first hop is certain only of a show with no such link, so any
/// one is the strip.
fn named_from_the_other_side(anchor: &str, known: &BTreeMap<String, FranchiseListing>) -> bool {
    known
        .iter()
        .filter(|(id, _)| id.as_str() != anchor)
        .any(|(_, l)| {
            l.relations.iter().any(|r| {
                r.kind == "anime"
                    && r.id == anchor
                    && matches!(r.role.as_str(), "prequel" | "sequel")
            })
        })
}

/// Whether `anchor`'s listing names an entry the walk's parent check
/// weighs: a parent story, a side story or a spin-off.
fn names_a_possible_parent(anchor: &str, listing: &FranchiseListing) -> bool {
    listing.relations.iter().any(|r| {
        r.kind == "anime"
            && r.id != anchor
            && matches!(r.role.as_str(), "parent_story" | "side_story" | "spinoff")
    })
}

/// What `anchor`'s own `listing` says the show page will draw, before
/// the parent check: a parent story it names counts for nothing here.
/// Any anime prequel or sequel is the strip, even one the show also
/// lists as its other version, which the walk leaves out of the run
/// (rule 3): the franchise's main story is still read along that
/// entry's links, and what they reach can put the show in a run.
fn from_own_listing(anchor: &str, listing: &FranchiseListing) -> FirstHop {
    let anime: Vec<&FranchiseRelation> = listing
        .relations
        .iter()
        .filter(|r| r.kind == "anime" && r.id != anchor)
        .collect();
    if anime
        .iter()
        .any(|r| matches!(r.role.as_str(), "prequel" | "sequel"))
    {
        return FirstHop::Strip;
    }
    let listed_beside = |r: &&&FranchiseRelation| {
        r.subtype.as_deref() != Some("music") && !NEVER_LISTED.contains(&r.role.as_str())
    };
    if anime.iter().any(|r| listed_beside(&r)) {
        FirstHop::Link
    } else {
        FirstHop::None
    }
}

/// Roles the franchise page never lists an entry under on their own.
const NEVER_LISTED: [&str; 3] = ["character", "adaptation", "alternative_setting"];

#[cfg(test)]
#[path = "franchise_walk_first_hop_test.rs"]
mod tests;
