//! Versions of one pick: the provider's copies of the same season
//! under names that differ only by a version tag — hianime lists
//! JoJo's seasons twice, once as "… (Uncensored)" and once without
//! the tag. The picker reports such copies as versions of its pick
//! rather than choosing between them silently; see "Versions of one
//! pick" in `docs/title-resolution.md`.

use serde::{Deserialize, Serialize};

use crate::scraper::provider::{BrowseHit, EpisodeRef};

pub(crate) use super::play_native_version_copies::{copy_died, note_dead, versions_of};

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

impl VersionKind {
    /// The kind as the wire names it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Uncensored => "uncensored",
        }
    }
}

/// A version as the answers that carry a pick name it: its kind and
/// the show key it plays under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionInfo {
    /// Which copy it is.
    pub kind: VersionKind,
    /// The show key the copy plays under (`hianime:<slug>`).
    pub show_key: String,
    /// Whether the copy carries the audio mode an availability answer
    /// was asked about; absent where nobody asked, or nobody answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_mode: Option<bool>,
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
    pub fn choose(mut self, version: Option<VersionKind>) -> Self {
        let Some(chosen) = version
            .and_then(|kind| self.versions.iter().find(|v| v.kind == kind))
            .cloned()
        else {
            return self;
        };
        self.hit = chosen.hit;
        self.episodes = chosen.episodes;
        self
    }
}

/// The versions of `picked` as the answers name them, each under its
/// show key with `provider`, and the one it plays — nothing for a pick
/// with one version.
#[must_use]
pub fn named_versions(
    provider: crate::scraper::provider::ProviderId,
    picked: &crate::commands::play_native::PickedShow,
) -> (Vec<VersionInfo>, Option<VersionKind>) {
    let infos = picked
        .versions
        .iter()
        .map(|v| VersionInfo {
            kind: v.kind,
            show_key: crate::scraper::provider::ShowKey::new(provider, v.hit.slug.clone())
                .to_string(),
            has_mode: None,
        })
        .collect();
    let playing = picked
        .versions
        .iter()
        .find(|v| v.hit.slug == picked.hit.slug)
        .map(|v| v.kind);
    (infos, playing)
}

/// `versions` with each listing's rows numbered and tagged as `fitted`
/// — the pick's listing after a reading re-tagged it — numbers them,
/// row by row: a version is the same episodes under its own ids, so a
/// reading of the pick's listing is a reading of each. A version whose
/// listing is not the pick's length, or whose rows are not numbered by
/// position, is left as it is.
#[must_use]
pub(crate) fn fitted_versions(versions: &[ShowVersion], fitted: &[EpisodeRef]) -> Vec<ShowVersion> {
    versions
        .iter()
        .map(|v| {
            // Only a copy numbered by position, as the reading needs the
            // pick's to be, takes the pick's numbering.
            let positional = v
                .episodes
                .iter()
                .zip(1u32..)
                .all(|(e, slot)| e.number == slot && e.number2.is_none());
            if v.episodes.len() != fitted.len() || !positional {
                return v.clone();
            }
            ShowVersion {
                episodes: v
                    .episodes
                    .iter()
                    .zip(fitted)
                    .map(|(own, f)| EpisodeRef {
                        number: f.number,
                        number2: f.number2.clone(),
                        ..own.clone()
                    })
                    .collect(),
                ..v.clone()
            }
        })
        .collect()
}

/// `picked` with its `versions`, playing the uncensored one when it
/// has versions.
pub(crate) fn with_versions(
    mut picked: crate::commands::play_native::PickedShow,
    versions: Vec<ShowVersion>,
) -> crate::commands::play_native::PickedShow {
    picked.versions = versions;
    picked.choose(Some(VersionKind::Uncensored))
}

#[cfg(test)]
#[path = "play_native_versions_test.rs"]
mod tests;

#[cfg(test)]
#[path = "play_native_versions_flow_test.rs"]
mod flow_tests;
