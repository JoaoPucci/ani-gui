//! A special the tracker has no entry for, filed among the extras a
//! related entry counts: Unlimited Blade Works' prologue (Kitsu 9718),
//! which AniList has no entry for, is the "Episode 0" AniList's 19603
//! lists before the TV season's twelve.
//!
//! The rule reads the related entry's own map, built by the other rules
//! from that entry's evidence, so the two never disagree about where the
//! extra is.

use serde::{Deserialize, Serialize};

use super::episode_map::{EpisodeMap, KitsuShow, TrackerShow};

/// One entry Kitsu relates the special to that resolves on the tracker:
/// the Kitsu entry, the tracker entry it resolves to, that entry's map
/// for it, and whether another of its Kitsu relations could be the same
/// extras.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedFiling {
    /// The related Kitsu entry.
    pub related: KitsuShow,
    /// The tracker entry it resolves to.
    pub entry: TrackerShow,
    /// The related entry's map on that tracker entry.
    pub map: EpisodeMap,
    /// Whether another Kitsu entry related to it, one the tracker has no
    /// entry for either, counts the special's episodes too.
    pub contested: bool,
}

/// What the rule reads for a special the tracker has no entry for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecialEvidence {
    /// The special, as Kitsu has it.
    pub special: KitsuShow,
    /// Every related Kitsu entry that resolves on the tracker.
    pub related: Vec<RelatedFiling>,
}

/// The special's map, where the evidence files it among a related
/// entry's extras; `None` where it does not.
#[must_use]
pub fn special_extras(_evidence: &SpecialEvidence) -> Option<EpisodeMap> {
    None
}

/// `existing`, the map the other rules gave the special, together with
/// the special rule's.
#[must_use]
pub fn with_special(existing: EpisodeMap, _special: Option<EpisodeMap>) -> EpisodeMap {
    existing
}
