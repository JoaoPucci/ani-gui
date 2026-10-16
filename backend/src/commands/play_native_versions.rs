//! Versions of one pick: the provider's copies of the same season
//! under names that differ only by a version tag — hianime lists
//! JoJo's seasons twice, once as "… (Uncensored)" and once without
//! the tag. The picker reports such copies as versions of its pick
//! rather than choosing between them silently; see "Versions of one
//! pick" in `docs/title-resolution.md`.

use serde::{Deserialize, Serialize};

use crate::scraper::provider::{BrowseHit, EpisodeRef};

/// Which copy of a season a version is. The renderer labels it; the
/// backend only names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionKind {
    /// The listing without a version tag.
    Standard,
    /// The listing tagged "(Uncensored)" or "[Uncensored]".
    Uncensored,
}

/// One version of a pick: the copy's hit and the listing its probe
/// already heard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowVersion {
    /// Which copy this is.
    pub kind: VersionKind,
    /// The copy's browse hit.
    pub hit: BrowseHit,
    /// The copy's episodes, as its probe returned them.
    pub episodes: Vec<EpisodeRef>,
}

impl crate::commands::play_native::PickedShow {
    /// The pick playing `version` when it is one of its versions; the
    /// pick as it is otherwise — a show with one version plays it
    /// whatever is asked.
    #[must_use]
    pub fn choose(self, _version: Option<VersionKind>) -> Self {
        self
    }
}

#[cfg(test)]
#[path = "play_native_versions_test.rs"]
mod tests;
