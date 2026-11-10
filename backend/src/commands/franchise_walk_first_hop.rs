//! The walk's first hop: what the show page can tell from the anchor's
//! own relation listing, the first listing the walk reads, before the
//! walk has read anything else.
//!
//! The show page reserves room for its watch-order strip (or its lone
//! franchise link) while the walk runs, so the strip does not push the
//! page down when it lands. [`first_hop`] says which of the two to
//! reserve, or that nothing is coming, by the rules the walk applies to
//! the same listing:
//!
//! - **strip**: the show lists an anime prequel or sequel other than
//!   itself that it does not also list as its alternative version or
//!   setting (rule 3 leaves those out of the run), or an anime parent
//!   story, whose run a side story alone takes;
//! - **link**: else, any anime relation the franchise page lists beside
//!   the show: anything but a music video, a character crossover, an
//!   adaptation or an alternative setting;
//! - **none**: else.
//!
//! Only the strip can be wrong, and only from what the show's listing
//! cannot see: a sequel whose own listing names the show as its other
//! version, or sequels that are versions of one another. The walk then
//! draws the link or nothing, and the page's placeholder gives way to
//! it. Where the first hop says link or nothing, the walk agrees.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

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
    let _ = details;
    let Some(listing) = known.get(anchor) else {
        return Step::Read {
            listings: vec![anchor.to_string()],
            details: Vec::new(),
        };
    };
    Step::Done(from_own_listing(anchor, listing))
}

/// What `anchor`'s own `listing` says the show page will draw.
fn from_own_listing(anchor: &str, listing: &FranchiseListing) -> FirstHop {
    let anime: Vec<&FranchiseRelation> = listing
        .relations
        .iter()
        .filter(|r| r.kind == "anime" && r.id != anchor)
        .collect();
    let versions: BTreeSet<&str> = anime
        .iter()
        .filter(|r| ALTERNATIVES.contains(&r.role.as_str()))
        .map(|r| r.id.as_str())
        .collect();
    let leads_on = |r: &&&FranchiseRelation| match r.role.as_str() {
        "prequel" | "sequel" => !versions.contains(r.id.as_str()),
        role => role == "parent_story",
    };
    if anime.iter().any(|r| leads_on(&r)) {
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

/// Roles that make an entry another version or setting of the show.
const ALTERNATIVES: [&str; 2] = ["alternative_version", "alternative_setting"];

/// Roles the franchise page never lists an entry under on their own.
const NEVER_LISTED: [&str; 3] = ["character", "adaptation", "alternative_setting"];

#[cfg(test)]
#[path = "franchise_walk_first_hop_test.rs"]
mod tests;
