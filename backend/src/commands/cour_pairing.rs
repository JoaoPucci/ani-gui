//! The cross-cour verdicts on a Kitsu entry the app already holds —
//! the mapping guard's and the reverse resolve's — split from
//! `kitsu.rs` for the per-file complexity bar. The parsers they read
//! live in [`super::cour`]; the name comparison that overrides them
//! in [`super::cour_same_name`].

use crate::meta::kitsu::KitsuAnimeRef;

/// Whether `detail` names another cour than `provider_cour`, the cour
/// `show_title` carries. An entry one of whose own titles is
/// `show_title` ([`super::cour_same_name::names_the_entry`]) names no
/// other cour, whatever its slug. Otherwise the slug decides: Kitsu's
/// convention leaves cour 1 unmarked, so a slug without a suffix is
/// cour 1; a detail without a slug is no evidence.
pub(crate) fn entry_disagrees(
    show_title: &str,
    provider_cour: u32,
    detail: &KitsuAnimeRef,
) -> bool {
    // The name is read as the cour is, without a legacy cache row's
    // "(<N> episodes)" tail.
    let name = super::cour::strip_trailing_episode_count(show_title.trim_end());
    if super::cour_same_name::names_the_entry(name, entry_names(detail)) {
        return false;
    }
    detail
        .slug
        .as_deref()
        .map(|slug| super::cour::cour_from_slug(slug).unwrap_or(1))
        .is_some_and(|kitsu_cour| kitsu_cour != provider_cour)
}

/// Whether the reverse resolve may take `hit` for a source whose cour
/// is `source_cour`, searched for as `term`: a hit one of whose own
/// titles is the term, or one whose slug agrees with the cour
/// ([`super::cour::hit_cour_disagrees`]).
pub(crate) fn hit_agrees(source_cour: Option<u32>, term: &str, hit: &KitsuAnimeRef) -> bool {
    super::cour_same_name::names_the_entry(term, entry_names(hit))
        || !super::cour::hit_cour_disagrees(source_cour, hit.slug.as_deref())
}

/// Every name the entry carries: the canonical title, the localized
/// titles and the abbreviations.
fn entry_names(detail: &KitsuAnimeRef) -> impl Iterator<Item = &str> {
    std::iter::once(detail.canonical_title.as_str())
        .chain(detail.titles.values().map(String::as_str))
        .chain(detail.abbreviated_titles.iter().map(String::as_str))
}

#[cfg(test)]
#[path = "cour_pairing_test.rs"]
mod tests;
