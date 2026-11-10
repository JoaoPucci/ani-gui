//! A side-story anchor's parent stories, checked against Kitsu listing
//! a pair the wrong way round: One Piece names LEGO ONE PIECE, its
//! two-episode spin-off, as its parent story, and LEGO names One Piece
//! as its side story. An entry is the anchor's parent story only when
//! it is at least as much a hub as the anchor (its listing names at
//! least as many side stories, spin-offs and recaps); an entry the
//! anchor names as its side story or spin-off is its parent story
//! instead when it started before the anchor and names more of them.

use std::collections::{BTreeMap, BTreeSet};

use super::alt::listed;
use super::{order_key, Lookup};
use crate::meta::kitsu::FranchiseRelation;

/// Roles under which a listing names an entry beneath it.
const BENEATH: [&str; 3] = ["side_story", "spinoff", "summary"];

/// How many entries `id`'s listing names beneath it, when read.
fn beneath(look: &Lookup<'_>, id: &str) -> Option<usize> {
    look.listing(id)?;
    let ids: BTreeSet<&str> = listed(look, id, &BENEATH).map(|r| r.id.as_str()).collect();
    Some(ids.len())
}

/// The anchor's parent stories, earliest first: those it names as its
/// parent story that name at least as many entries beneath them as it
/// does, and those it names as its side story or spin-off that started
/// before it and name more.
///
/// # Errors
/// The candidates' listings to read first.
pub(super) fn parent_stories(anchor: &str, look: &Lookup<'_>) -> Result<Vec<String>, Vec<String>> {
    let start = |r: &FranchiseRelation| r.entry.as_ref().and_then(|e| e.start.clone());
    let own_start = look.entry(anchor, &BTreeSet::new()).and_then(|e| e.start);
    let named: BTreeMap<&str, Option<String>> = listed(look, anchor, &["parent_story"])
        .map(|r| (r.id.as_str(), start(r)))
        .collect();
    let earlier: BTreeMap<&str, Option<String>> = listed(look, anchor, &["side_story", "spinoff"])
        .filter(|r| matches!((start(r), &own_start), (Some(a), Some(b)) if a < *b))
        .map(|r| (r.id.as_str(), start(r)))
        .collect();
    let unread: Vec<String> = named
        .keys()
        .chain(earlier.keys())
        .filter(|id| look.listing(id).is_none())
        .map(|id| (*id).to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !unread.is_empty() {
        return Err(unread);
    }
    let own = beneath(look, anchor).unwrap_or(0);
    let size = |id: &str| beneath(look, id).unwrap_or(0);
    let mut parents: Vec<(&str, Option<String>)> = named
        .iter()
        .filter(|(id, _)| size(id) >= own)
        .chain(earlier.iter().filter(|(id, _)| size(id) > own))
        .map(|(id, s)| (*id, s.clone()))
        .collect();
    parents.sort_by_key(|(id, s)| order_key(s.as_deref(), id));
    parents.dedup_by_key(|(id, _)| *id);
    Ok(parents.into_iter().map(|(id, _)| id.to_string()).collect())
}
