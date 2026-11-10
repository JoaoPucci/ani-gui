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
}
