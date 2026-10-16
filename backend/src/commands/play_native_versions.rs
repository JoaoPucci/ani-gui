//! Versions of one pick: the provider's copies of the same season
//! under names that differ only by a version tag — hianime lists
//! JoJo's seasons twice, once as "… (Uncensored)" and once without
//! the tag. The picker reports such copies as versions of its pick
//! rather than choosing between them silently; see "Versions of one
//! pick" in `docs/title-resolution.md`.

use serde::{Deserialize, Serialize};

use crate::scraper::provider::{BrowseHit, EpisodeRef};

use super::play_native_title_grammar::normalized;

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

/// A version as the answers that carry a pick name it: its kind and
/// the show key it plays under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionInfo {
    /// Which copy it is.
    pub kind: VersionKind,
    /// The show key the copy plays under (`hianime:<slug>`).
    pub show_key: String,
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

/// The version tag's word, as a bracket holds it.
const UNCENSORED: &str = "uncensored";

/// `title` read for its version: the title without a bracketed
/// version tag — "(Uncensored)", "[UNCENSORED]", in ASCII or
/// full-width brackets, any case — compared as written
/// ([`normalized`]: case and spacing aside, catalogue tags kept), and
/// which version the tag names.
pub(crate) fn version_of(title: &str) -> (String, VersionKind) {
    let mut rest = title;
    let mut out = String::new();
    let mut kind = VersionKind::Standard;
    while let Some(open) = rest.find(['(', '[', '（', '［', '【']) {
        let after = &rest[open..];
        let inner_start = after.chars().next().map_or(1, char::len_utf8);
        let Some(close) = after.find([')', ']', '）', '］', '】']) else {
            break;
        };
        let inner: String = after[inner_start..close]
            .trim()
            .chars()
            .map(|c| match c {
                '！'..='～' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
                _ => c,
            })
            .collect();
        let closer = after[close..].chars().next().map_or(1, char::len_utf8);
        if inner.eq_ignore_ascii_case(UNCENSORED) {
            kind = VersionKind::Uncensored;
            out.push_str(&rest[..open]);
            out.push(' ');
        } else {
            out.push_str(&rest[..open + close + closer]);
        }
        rest = &after[close + closer..];
    }
    out.push_str(rest);
    (normalized(&out), kind)
}

/// Whether two hits list one title under any version tag, as far as
/// their cards tell: the same title once the tag is set aside, and the
/// same badge.
fn same_title(a: &BrowseHit, b: &BrowseHit) -> bool {
    version_of(&a.title).0 == version_of(&b.title).0 && a.kind == b.kind
}

/// Whether two hits are copies of one listing under different version
/// tags: [`same_title`], another version.
fn copies(a: &BrowseHit, b: &BrowseHit) -> bool {
    same_title(a, b) && version_of(&a.title).1 != version_of(&b.title).1
}

/// The rows of a listing as a viewer reads them: each episode's number
/// and tag.
fn numbering(episodes: &[EpisodeRef]) -> Vec<(u32, Option<&str>)> {
    episodes
        .iter()
        .map(|e| (e.number, e.number2.as_deref()))
        .collect()
}

/// The versions of the winner at `winner` among the probed candidates
/// `probed` (hit, listing, distance, year confirmed), in the
/// provider's order: every candidate that could be picked at the best
/// distance `best` — the winner's — whose card lists the winner's
/// title ([`same_title`]) and whose listing numbers the same episodes.
/// Empty unless they are under different version tags, one each; two
/// listings under one tag cannot be told apart, and the pick stays as
/// it is.
pub(crate) fn versions_of(
    probed: &[(&BrowseHit, Vec<EpisodeRef>, u32, bool)],
    winner: usize,
    best: u32,
) -> Vec<ShowVersion> {
    let (w, w_eps, _, _) = &probed[winner];
    let rows = numbering(w_eps);
    let group: Vec<ShowVersion> = probed
        .iter()
        .enumerate()
        .filter(|(i, (h, eps, d, _))| {
            *i == winner || (*d == best && same_title(w, h) && numbering(eps) == rows)
        })
        .map(|(_, (h, eps, _, _))| ShowVersion {
            kind: version_of(&h.title).1,
            hit: (*h).clone(),
            episodes: eps.clone(),
        })
        .collect();
    let distinct = group
        .iter()
        .enumerate()
        .all(|(i, v)| group[..i].iter().all(|u| u.kind != v.kind));
    if group.len() < 2 || !distinct {
        return Vec::new();
    }
    group
}

/// Whether a candidate whose probe died unheard (`dead`, those that
/// could have been picked) copies `winner` under another version tag:
/// the pick's versions cannot be told then, and playing one copy now
/// and the other once the dead one answers would split the show's
/// history across two keys on the strength of weather.
pub(crate) fn copy_died(dead: &[&BrowseHit], winner: &BrowseHit) -> bool {
    dead.iter().any(|d| copies(d, winner))
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
