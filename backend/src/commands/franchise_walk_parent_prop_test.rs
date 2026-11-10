//! Properties of a side-story anchor's parent stories over random
//! catalogues: which entry the walk places the anchor under.

use std::collections::BTreeSet;

use proptest::prelude::*;

use super::prop_tests::{catalogue, drive_with};
use super::rules::listed;
use super::*;

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
        let own = beneath(&cat.listings, &anchor);
        let start = |id: &str| cat.entries[id].start.clone();
        for p in f.parents.get(&anchor).into_iter().flatten() {
            let earlier = matches!((start(p), start(&anchor)), (Some(a), Some(b)) if a < b);
            prop_assert!(
                earlier || beneath(&cat.listings, p) >= own,
                "{} placed under {}", anchor, p
            );
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
        let named = listed(&look, &anchor, &["parent_story"]).next().is_some();
        let earlier = listed(&look, &anchor, &["side_story", "spinoff"])
            .any(|r| matches!((start(&r.id), start(&anchor)), (Some(a), Some(b)) if a < b));
        if !named && !earlier {
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
        let named: BTreeSet<String> = listed(&look, &anchor, &["parent_story"])
            .map(|r| r.id.clone())
            .collect();
        for p in f.parents.get(&anchor).into_iter().flatten() {
            if named.contains(p) {
                continue;
            }
            prop_assert!(
                listed(&look, p, &["parent_story"]).any(|r| r.id == anchor),
                "{} placed under {}, which does not name it back", anchor, p
            );
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
