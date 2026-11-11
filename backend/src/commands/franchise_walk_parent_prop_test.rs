//! Properties of a side-story anchor's parent stories over random
//! catalogues: which entry the walk places the anchor under.
//!
//! The parent check runs on the anchor, and, where the anchor's own
//! run is a run of one-offs and the anchor names no parent story that
//! check takes, on the run's other members
//! (`franchise_walk_run.rs`): each property holds for the entry whose
//! check placed the anchor, so it is asked of the anchor or of one of
//! those members ([`placers`]).

use std::collections::BTreeSet;

use proptest::prelude::*;

use super::parents::parent_stories;
use super::prop_tests::{catalogue, drive_with};
use super::rules::listed;
use super::walking::walk;
use super::*;

/// Whether `e` is a series: a TV series, OVA or ONA not of one episode.
fn is_series(e: &FranchiseEntry) -> bool {
    matches!(e.subtype.as_deref(), Some("TV" | "OVA" | "ONA")) && e.episodes != Some(1)
}

/// The entries whose parent check placed `anchor`, as the walk picks
/// them: the anchor, unless its own run is a run of one-offs and the
/// anchor names no parent story outside it that the check takes, in
/// which case the run's members.
fn placers(cat: &super::tests::Catalogue, anchor: &str) -> Vec<String> {
    let look = Lookup {
        known: &cat.listings,
        details: &cat.entries,
    };
    let own = walk(anchor, &look, BTreeSet::new()).expect("every listing is known");
    let one_offs = own.members.len() > 1 && !own.members.keys().any(|m| is_series(&cat.entries[m]));
    if !one_offs {
        return vec![anchor.to_string()];
    }
    let mine = parent_stories(anchor, &look, &BTreeSet::new()).expect("every listing is known");
    if mine.parents.iter().any(|p| !own.members.contains_key(p)) {
        return vec![anchor.to_string()];
    }
    own.members.keys().cloned().collect()
}

/// The anime `id`'s listing names beneath it: its side stories,
/// spin-offs and recaps.
fn beneath(known: &BTreeMap<String, FranchiseListing>, id: &str) -> usize {
    let details = BTreeMap::new();
    let look = Lookup {
        known,
        details: &details,
    };
    listed(&look, id, &["side_story", "spinoff", "summary"])
        .map(|r| r.id.clone())
        .collect::<BTreeSet<_>>()
        .len()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A side-story anchor is placed only under entries that started
    /// before it or name at least as many side stories, spin-offs and
    /// recaps as it does: a later entry the anchor names as its parent
    /// story that has fewer is the anchor's own spin-off, listed the
    /// wrong way round.
    #[test]
    fn an_anchor_is_never_placed_under_a_later_smaller_entry((cat, anchor) in catalogue()) {
        let (f, _) = drive_with(&anchor, &cat, &[]);
        let start = |id: &str| cat.entries[id].start.clone();
        let by = placers(&cat, &anchor);
        for p in f.parents.get(&anchor).into_iter().flatten() {
            let fits = by.iter().any(|x| {
                let earlier = matches!((start(p), start(x)), (Some(a), Some(b)) if a < b);
                earlier || beneath(&cat.listings, p) >= beneath(&cat.listings, x)
            });
            prop_assert!(fits, "{} placed under {}", anchor, p);
        }
    }

    /// An anchor that names nothing as its parent story and no side
    /// story or spin-off that started before it is never placed under
    /// another entry.
    #[test]
    fn an_anchor_with_no_candidate_parent_is_not_placed((cat, anchor) in catalogue()) {
        let (f, _) = drive_with(&anchor, &cat, &[]);
        let details = BTreeMap::new();
        let look = Lookup { known: &cat.listings, details: &details };
        let start = |id: &str| cat.entries[id].start.clone();
        let candidate = |x: &String| {
            listed(&look, x, &["parent_story"]).next().is_some()
                || listed(&look, x, &["side_story", "spinoff"])
                    .any(|r| matches!((start(&r.id), start(x)), (Some(a), Some(b)) if a < b))
        };
        if !placers(&cat, &anchor).iter().any(candidate) {
            prop_assert!(!f.parents.contains_key(&anchor));
        }
    }

    /// A parent the anchor names only as its own side story or spin-off
    /// lists the anchor back as its parent story: the pair is listed
    /// the wrong way round from both sides, not from one.
    #[test]
    fn a_reversed_parent_names_the_anchor_back((cat, anchor) in catalogue()) {
        let (f, _) = drive_with(&anchor, &cat, &[]);
        let details = BTreeMap::new();
        let look = Lookup { known: &cat.listings, details: &details };
        let by = placers(&cat, &anchor);
        for p in f.parents.get(&anchor).into_iter().flatten() {
            let fits = by.iter().any(|x| {
                listed(&look, x, &["parent_story"]).any(|r| &r.id == p)
                    || listed(&look, p, &["parent_story"]).any(|r| &r.id == x)
            });
            prop_assert!(fits, "{} placed under {}, which does not name it back", anchor, p);
        }
    }

    /// Every candidate the parent check considers had its listing read,
    /// so one read only in part leaves the answer incomplete, taken or
    /// not.
    #[test]
    fn a_candidate_read_in_part_leaves_the_answer_incomplete((cat, anchor) in catalogue()) {
        let (f, (known, _)) = drive_with(&anchor, &cat, &[]);
        let details = BTreeMap::new();
        let look = Lookup { known: &cat.listings, details: &details };
        let alone = f.entries.len() == 1 && f.entries[0].id == anchor;
        if !(alone || f.parents.contains_key(&anchor)) {
            return Ok(());
        }
        let candidates = listed(&look, &anchor, &["parent_story", "side_story", "spinoff"])
            .map(|r| r.id.clone())
            .filter(|id| known.contains_key(id));
        for c in candidates {
            if !cat.listings[&c].complete {
                prop_assert!(!f.complete, "{} read {} in part", anchor, c);
            }
        }
    }
}
