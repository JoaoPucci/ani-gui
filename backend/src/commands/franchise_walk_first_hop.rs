//! The walk's first hop: what the show page can tell from the anchor's
//! own relation listing, the first listing the walk reads, before the
//! walk has read anything else.
//!
//! The show page reserves room for its watch-order strip (or its lone
//! franchise link) while the walk runs, so the strip does not push the
//! page down when it lands. [`first_hop`] says which of the two to
//! reserve, or that nothing is coming.

use serde::{Deserialize, Serialize};

use crate::meta::kitsu::FranchiseListing;

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

/// What `anchor`'s own `listing` says the show page will draw.
#[must_use]
pub fn first_hop(anchor: &str, listing: &FranchiseListing) -> FirstHop {
    let _ = (anchor, listing);
    FirstHop::None
}

#[cfg(test)]
#[path = "franchise_walk_first_hop_test.rs"]
mod tests;
