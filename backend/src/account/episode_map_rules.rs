//! The rules that build an [`EpisodeMap`] from [`Evidence`].
//!
//! Each rule explains one way a tracker's count can differ from Kitsu's,
//! and fires only on evidence that settles it; the map falls back to
//! identity — every episode sent as it is — whenever nothing conclusive
//! explains the difference, since a wrong mapping is worse than none.
//! `docs/title-resolution.md` ("Episode numbers") tables the cases.

use super::episode_map::{EpisodeMap, Evidence, Identity};

/// The map for `evidence`.
#[must_use]
pub fn map_show(evidence: &Evidence) -> EpisodeMap {
    let (Some(kitsu_count), Some(tracker_total)) = (
        evidence.kitsu.episodes.filter(|n| *n > 0),
        evidence.entry.episodes.filter(|n| *n > 0),
    ) else {
        return EpisodeMap::identity(Identity::CountUnknown);
    };
    if kitsu_count == tracker_total {
        return EpisodeMap::identity(Identity::CountsAgree);
    }
    EpisodeMap::identity(Identity::Unexplained)
}

#[cfg(test)]
#[path = "episode_map_test.rs"]
mod tests;
