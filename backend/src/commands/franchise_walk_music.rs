//! Music videos are no part of a show's franchise. Kitsu relates a
//! show to the songs made for it (an opening's video, a film's theme),
//! in the listings recorded here as `other`, `character` or
//! `side_story`, and nothing keeps it from relating one as a prequel
//! or sequel. A music video is no story entry, so no show's franchise
//! answer names one: not as its root, in its main story or a page's
//! chain, as the entry it is named from, as a parent or in a section.
//! And most of the music videos Kitsu lists cannot be watched here: the
//! provider lists 81, of which about a quarter play.
//!
//! The walk therefore reads every listing without its music videos,
//! as if Kitsu had not related them, and walks through none: two
//! seasons linked only through a music video are not joined, since a
//! story link between them is one Kitsu lists between them. A music
//! video's own page relates to nothing: once its detail is read, the
//! walk answers with the music video alone.

use std::collections::BTreeMap;

use crate::meta::kitsu::{FranchiseEntry, FranchiseListing, FranchiseRelation};

/// Whether a Kitsu `subtype` is a music video's.
fn is_music(subtype: Option<&str>) -> bool {
    subtype.is_some_and(|s| s.eq_ignore_ascii_case("music"))
}

/// Whether `entry` is a music video.
pub(crate) fn is_music_entry(entry: &FranchiseEntry) -> bool {
    is_music(entry.subtype.as_deref())
}

/// Whether `r` relates a music video.
fn relates_music(r: &FranchiseRelation) -> bool {
    is_music(r.subtype.as_deref()) || is_music(r.entry.as_ref().and_then(|e| e.subtype.as_deref()))
}

/// What the walk asks to read first, as (listings, details).
pub(crate) type Unread = (Vec<String>, Vec<String>);

/// The `known` listings as the walk reads them from `anchor`, or what
/// to read first: the anchor's listing and detail, together. A detail
/// that says music video answers at once: its page relates to nothing,
/// whatever its listing holds, and the walk sees it alone. Any other
/// page's listings are read with their music videos dropped.
///
/// # Errors
/// The anchor's listing and detail, whichever are unread.
pub(crate) fn read_for(
    anchor: &str,
    known: &BTreeMap<String, FranchiseListing>,
    details: &BTreeMap<String, FranchiseEntry>,
) -> Result<BTreeMap<String, FranchiseListing>, Unread> {
    if details.get(anchor).is_some_and(is_music_entry) {
        let alone = FranchiseListing {
            relations: Vec::new(),
            complete: true,
        };
        return Ok(BTreeMap::from([(anchor.to_string(), alone)]));
    }
    let unread = |has: bool| {
        if has {
            Vec::new()
        } else {
            vec![anchor.to_string()]
        }
    };
    let (listings, needed) = (
        unread(known.contains_key(anchor)),
        unread(details.contains_key(anchor)),
    );
    if !listings.is_empty() || !needed.is_empty() {
        return Err((listings, needed));
    }
    Ok(without_music(known))
}

/// `known` with every music video dropped from each listing.
pub(crate) fn without_music(
    known: &BTreeMap<String, FranchiseListing>,
) -> BTreeMap<String, FranchiseListing> {
    known
        .iter()
        .map(|(id, listing)| {
            let relations = listing
                .relations
                .iter()
                .filter(|r| !relates_music(r))
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
