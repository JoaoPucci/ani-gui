//! A side-story anchor's parent stories, checked against Kitsu listing
//! a pair the wrong way round: One Piece names LEGO ONE PIECE, its
//! two-episode spin-off, as its parent story, and LEGO names One Piece
//! as its side story. An entry the anchor names as its parent story is
//! refused only when it did not start before the anchor and is less of
//! a hub (its listing names fewer side stories, spin-offs and recaps).
//! An entry the anchor names as its side story or spin-off is its
//! parent story instead when it started before the anchor, names the
//! anchor back as its parent story, and names more of them. Every
//! candidate's listing read to decide counts as read: toward the
//! walk's bound on what it reads after, and toward whether the answer
//! is complete. The candidates are read only as far as the bound leaves
//! room (`franchise_walk_candidates.rs`); a candidate past it is not
//! decided, and the answer is incomplete.

use std::collections::{BTreeMap, BTreeSet};

use super::alt::listed;
use super::candidates::{candidates, to_read};
use super::{order_key, Lookup};

/// Roles under which a listing names an entry beneath it.
const BENEATH: [&str; 3] = ["side_story", "spinoff", "summary"];

/// How many entries `id`'s listing names beneath it, when read.
fn beneath(look: &Lookup<'_>, id: &str) -> Option<usize> {
    look.listing(id)?;
    let ids: BTreeSet<&str> = listed(look, id, &BENEATH).map(|r| r.id.as_str()).collect();
    Some(ids.len())
}

/// What the parent check found: the parent stories, earliest first,
/// every candidate whose listing it read to decide, and whether the
/// bound left room to read every candidate.
pub(super) struct Found {
    pub(super) parents: Vec<String>,
    pub(super) read: BTreeSet<String>,
    pub(super) complete: bool,
}

/// The anchor's parent stories: those it names as its parent story
/// that started before it or name at least as many entries beneath
/// them as it does, and
/// those it names as its side story or spin-off that started before
/// it, name it back as their parent story, and name more.
///
/// The anchor's start is read from its details or, for an entry whose
/// details the walk never reads (another member of the anchor's run),
/// from the `consulted` listings that relate it. Only the candidates
/// the bound leaves room for, beside the listings already `counted`
/// toward it, are read and decided (`to_read`).
///
/// # Errors
/// The candidates' listings to read first.
pub(super) fn parent_stories(
    anchor: &str,
    look: &Lookup<'_>,
    consulted: &BTreeSet<String>,
    counted: &BTreeSet<String>,
) -> Result<Found, Vec<String>> {
    let (named, earlier, own_start) = candidates(anchor, look, consulted);
    let (read, cut) = to_read(anchor, look, consulted, counted);
    let named: BTreeMap<&str, Option<String>> = named
        .into_iter()
        .filter(|(id, _)| read.contains(*id))
        .collect();
    let earlier: BTreeMap<&str, Option<String>> = earlier
        .into_iter()
        .filter(|(id, _)| read.contains(*id))
        .collect();
    let unread: Vec<String> = read
        .iter()
        .filter(|id| look.listing(id).is_none())
        .cloned()
        .collect();
    if !unread.is_empty() {
        return Err(unread);
    }
    let own = beneath(look, anchor).unwrap_or(0);
    let size = |id: &str| beneath(look, id).unwrap_or(0);
    let names_back = |id: &str| listed(look, id, &["parent_story"]).any(|r| r.id == anchor);
    let before = |s: &Option<String>| matches!((s, &own_start), (Some(a), Some(b)) if a < b);
    let mut parents: Vec<(&str, Option<String>)> = named
        .iter()
        .filter(|(id, s)| before(s) || size(id) >= own)
        .chain(
            earlier
                .iter()
                .filter(|(id, _)| names_back(id) && size(id) > own),
        )
        .map(|(id, s)| (*id, s.clone()))
        .collect();
    parents.sort_by_key(|(id, s)| order_key(s.as_deref(), id));
    parents.dedup_by_key(|(id, _)| *id);
    Ok(Found {
        parents: parents.into_iter().map(|(id, _)| id.to_string()).collect(),
        read,
        complete: !cut,
    })
}
