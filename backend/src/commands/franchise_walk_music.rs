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
//! video's own page relates to nothing: the walk reads its detail
//! first, never its listing, and answers with the music video alone.

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

/// What the walk asks to read first, as (listings, details).
pub(crate) type Unread = (Vec<String>, Vec<String>);

/// The `known` listings as the walk reads them from `anchor`, or what
/// to read first. The anchor's detail comes first: a music video's
/// page relates to nothing, so its listing is never read, and the walk
/// sees it alone. Any other page reads its listing next, and every
/// listing is read with its music videos dropped.
///
/// # Errors
/// The anchor's detail, else its listing, while either is unread.
pub(crate) fn read_for(
    anchor: &str,
    known: &BTreeMap<String, FranchiseListing>,
    details: &BTreeMap<String, FranchiseEntry>,
) -> Result<BTreeMap<String, FranchiseListing>, Unread> {
    let Some(entry) = details.get(anchor) else {
        return Err((Vec::new(), vec![anchor.to_string()]));
    };
    if is_music(entry.subtype.as_deref()) {
        let alone = FranchiseListing {
            relations: Vec::new(),
            complete: true,
        };
        return Ok(BTreeMap::from([(anchor.to_string(), alone)]));
    }
    if !known.contains_key(anchor) {
        return Err((vec![anchor.to_string()], Vec::new()));
    }
    Ok(known
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
        .collect())
}
