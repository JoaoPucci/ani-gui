//! The shows a guessed history row can be linked to, and what to point
//! out before linking it to one — the pure half of
//! [`super::link_candidates`].
//!
//! The candidates are the hits of two Kitsu searches: the row's saved
//! title, which is what the guess searched, and the words of the row's
//! provider slug ([`crate::scraper::provider::key_search_term`]), which
//! the backend's own slug resolve searches. A slug often names what the
//! title leaves out — a year, a format — and the right show can rank
//! first there while the title's search does not list it at all. The
//! show the card shows now leads, then the two lists interleaved, each
//! show once; a music video never is one.
//!
//! The warnings are only that: the user decides. A season the row's
//! saved title names that the show does not, and an episode past the
//! show's count, are worth a second look before a link.

use serde::Serialize;

use crate::commands::cour::{cour_from_slug, cour_from_title};
use crate::meta::kitsu::KitsuAnimeRef;

/// What a link to a show would be worth a second look for: a stable
/// key the renderer words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkWarning {
    /// The row's saved title names a season the show does not.
    Season,
    /// The episode the card offers is past the show's count.
    Episode,
}

/// A show offered for a row, whether the card shows it now, and what a
/// link to it would be worth a second look for.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LinkCandidate {
    /// The show.
    #[serde(rename = "ref")]
    pub show: KitsuAnimeRef,
    /// The card shows this show now.
    pub current: bool,
    /// What to point out before linking the row to it.
    pub warnings: Vec<LinkWarning>,
}

/// The candidates' shows: `current` first, then `title_hits` and
/// `slug_hits` interleaved, each show once and no music video, with
/// whether each is `current`.
#[must_use]
pub fn merge_candidates(
    current: Option<&KitsuAnimeRef>,
    title_hits: &[KitsuAnimeRef],
    slug_hits: &[KitsuAnimeRef],
) -> Vec<(KitsuAnimeRef, bool)> {
    let longest = title_hits.len().max(slug_hits.len());
    let interleaved =
        (0..longest).flat_map(|i| title_hits.get(i).into_iter().chain(slug_hits.get(i)));
    let mut seen = std::collections::HashSet::new();
    current
        .into_iter()
        .chain(interleaved)
        .filter(|show| !is_music(show) && seen.insert(show.id.clone()))
        .map(|show| (show.clone(), current.is_some_and(|c| c.id == show.id)))
        .collect()
}

fn is_music(show: &KitsuAnimeRef) -> bool {
    show.subtype
        .as_deref()
        .is_some_and(|s| s.eq_ignore_ascii_case("music"))
}

/// The season a show names: the highest its titles or its slug carry,
/// 1 when none does.
#[must_use]
pub fn season_of(show: &KitsuAnimeRef) -> u32 {
    let by_title = std::iter::once(&show.canonical_title)
        .chain(show.titles.values())
        .filter_map(|t| cour_from_title(t));
    let by_slug = show.slug.as_deref().and_then(cour_from_slug);
    by_title.chain(by_slug).fold(1, u32::max)
}

/// What to point out before linking a row whose saved title is `title`
/// to `show`, the card offering `episode`: a season the title names
/// that the show does not, and an episode past the show's count.
#[must_use]
pub fn link_warnings(title: &str, show: &KitsuAnimeRef, episode: f64) -> Vec<LinkWarning> {
    let mut warnings = Vec::new();
    if cour_from_title(title).unwrap_or(1) != season_of(show) {
        warnings.push(LinkWarning::Season);
    }
    if show.episode_count.is_some_and(|n| episode > f64::from(n)) {
        warnings.push(LinkWarning::Episode);
    }
    warnings
}

/// The candidates for a row whose saved title is `title`, the card
/// offering `episode`: each show with whether it is `current_id`, and
/// its warnings.
#[must_use]
pub fn with_warnings(
    shows: Vec<(KitsuAnimeRef, bool)>,
    title: &str,
    episode: f64,
) -> Vec<LinkCandidate> {
    shows
        .into_iter()
        .map(|(show, current)| LinkCandidate {
            warnings: link_warnings(title, &show, episode),
            show,
            current,
        })
        .collect()
}

#[cfg(test)]
#[path = "link_rules_test.rs"]
mod tests;
