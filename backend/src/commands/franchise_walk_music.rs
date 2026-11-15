//! Music videos are no part of a franchise. Kitsu relates a show to
//! the songs made for it (an opening's video, a film's theme) under
//! any role, a prequel or sequel among them, but a music video is no
//! story entry, so no franchise answer names one: not its main story,
//! its name, a section, a parent or a page's chain. And most of the
//! music videos Kitsu lists cannot be watched here: the provider lists
//! 81, of which about a quarter play, so even as extras they would
//! mostly be titles that go nowhere.
//!
//! The walk therefore reads every listing without its music videos,
//! as if Kitsu had not related them, and walks through none: two
//! seasons linked only through a music video are not joined, since a
//! story link between them is one Kitsu lists between them. A music
//! video's own page relates to nothing, and its franchise is itself
//! alone.

use std::collections::BTreeMap;

use crate::meta::kitsu::{FranchiseEntry, FranchiseListing, FranchiseRelation};

/// Whether a Kitsu `subtype` is a music video's.
fn is_music(subtype: Option<&str>) -> bool {
    subtype.is_some_and(|s| s.eq_ignore_ascii_case("music"))
}

/// Whether `r` relates a music video.
fn relates_music(r: &FranchiseRelation) -> bool {
    is_music(r.subtype.as_deref()) || is_music(r.entry.as_ref().and_then(|e| e.subtype.as_deref()))
}

/// The `known` listings as the walk reads them: every music video
/// dropped from each, and nothing at all related to `anchor` when
/// `anchor` is one itself (its `details`).
pub(crate) fn without_music(
    anchor: &str,
    known: &BTreeMap<String, FranchiseListing>,
    details: &BTreeMap<String, FranchiseEntry>,
) -> BTreeMap<String, FranchiseListing> {
    let anchor_is_music = details
        .get(anchor)
        .is_some_and(|e| is_music(e.subtype.as_deref()));
    known
        .iter()
        .map(|(id, listing)| {
            let alone = anchor_is_music && id == anchor;
            let relations = listing
                .relations
                .iter()
                .filter(|r| !alone && !relates_music(r))
                .cloned()
                .collect();
            let read = FranchiseListing {
                relations,
                complete: listing.complete,
            };
            (id.clone(), read)
        })
        .collect()
}
