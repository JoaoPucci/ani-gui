//! The cross-cour verdicts on a Kitsu entry the app already holds —
//! the mapping guard's and the reverse resolve's — split from
//! `kitsu.rs` for the per-file complexity bar. The parsers they read
//! live in [`super::cour`].

use crate::meta::kitsu::KitsuAnimeRef;

/// Whether `detail` names another cour than `provider_cour`. Kitsu's
/// slug convention leaves cour 1 unmarked, so a slug without a suffix
/// is cour 1; a detail without a slug is no evidence.
pub(crate) fn entry_disagrees(provider_cour: u32, detail: &KitsuAnimeRef) -> bool {
    detail
        .slug
        .as_deref()
        .map(|slug| super::cour::cour_from_slug(slug).unwrap_or(1))
        .is_some_and(|kitsu_cour| kitsu_cour != provider_cour)
}

/// Whether the reverse resolve may take `hit` for a source whose cour
/// is `source_cour`: [`super::cour::hit_cour_disagrees`] on the hit's
/// slug.
pub(crate) fn hit_agrees(source_cour: Option<u32>, hit: &KitsuAnimeRef) -> bool {
    !super::cour::hit_cour_disagrees(source_cour, hit.slug.as_deref())
}
