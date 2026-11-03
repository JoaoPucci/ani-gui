//! Properties of the franchise walk over random catalogues.
//!
//! Every strategy returns a `BoxedStrategy`, for the Windows test
//! build's PDB limit (see `meta/kitsu_relations_prop_test.rs`).

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

use super::rules::{alt, run};
use super::tests::Catalogue;
use super::*;

const ROLES: [&str; 10] = [
    "prequel",
    "sequel",
    "prequel",
    "sequel",
    "alternative_version",
    "alternative_setting",
    "summary",
    "side_story",
    "parent_story",
    "character",
];
const SUBTYPES: [&str; 5] = ["TV", "movie", "OVA", "special", "music"];

/// An entry: its subtype, its start year (or none), whether its listing
/// is complete, and its relations as (role, target index).
type Spec = (usize, Option<u16>, bool, Vec<(usize, usize)>);

fn spec(n: usize) -> BoxedStrategy<Spec> {
    (
        0..SUBTYPES.len(),
        proptest::option::weighted(0.85, 1990u16..2030),
        prop::bool::weighted(0.95),
        prop::collection::vec((0..ROLES.len(), 0..n), 0..6),
    )
        .boxed()
}

fn catalogue() -> BoxedStrategy<(Catalogue, String)> {
    (2usize..14)
        .prop_flat_map(|n| (prop::collection::vec(spec(n), n), 0..n))
        .prop_map(|(specs, anchor)| {
            let id = |i: usize| (i + 1).to_string();
            let mut c = Catalogue::default();
            for (i, (subtype, year, _, _)) in specs.iter().enumerate() {
                let start = year.map(|y| format!("{y}-01-01"));
                c.add(&id(i), SUBTYPES[*subtype], start.as_deref());
            }
            for (i, (_, _, complete, rels)) in specs.iter().enumerate() {
                for (role, to) in rels {
                    c.rel(&id(i), ROLES[*role], &id(*to));
                }
                c.listings.get_mut(&id(i)).expect("listing").complete = *complete;
            }
            (c, id(anchor))
        })
        .boxed()
}

type Read = (
    BTreeMap<String, FranchiseListing>,
    BTreeMap<String, FranchiseEntry>,
);

/// Drive the walk from `anchor` with `prefetched` listings already
/// read, answering each need in reverse order; the result and what was
/// read.
fn drive_with(anchor: &str, cat: &Catalogue, prefetched: &[String]) -> (Franchise, Read) {
    let mut known: BTreeMap<String, FranchiseListing> = prefetched
        .iter()
        .map(|id| (id.clone(), cat.listings[id].clone()))
        .collect();
    let mut details = BTreeMap::new();
    for _ in 0..500 {
        match plan(anchor, &known, &details) {
            Plan::Done(f) => return (f, (known, details)),
            Plan::Need {
                listings,
                details: d,
            } => {
                assert!(!listings.is_empty() || !d.is_empty());
                for id in listings.into_iter().rev() {
                    assert!(!known.contains_key(&id), "asked again for {id}");
                    known.insert(id.clone(), cat.listings[&id].clone());
                }
                for id in d.into_iter().rev() {
                    assert!(!details.contains_key(&id), "asked again for {id}");
                    details.insert(id.clone(), cat.entries[&id].clone());
                }
            }
        }
    }
    panic!("the walk did not finish");
}

fn id_set(entries: &[FranchiseEntry]) -> BTreeSet<String> {
    entries.iter().map(|e| e.id.clone()).collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The answer depends on what the listings say, not on which were
    /// read first or what else was read beside them.
    #[test]
    fn the_walk_does_not_depend_on_read_order(
        (cat, anchor) in catalogue(),
        extra in prop::collection::vec(any::<prop::sample::Index>(), 0..6),
    ) {
        let (base, _) = drive_with(&anchor, &cat, &[]);
        let ids: Vec<String> = cat.listings.keys().cloned().collect();
        let prefetched: Vec<String> = extra.iter().map(|i| i.get(&ids).clone()).collect();
        let (again, _) = drive_with(&anchor, &cat, &prefetched);
        prop_assert_eq!(base, again);
    }

    /// The shape of the answer: the anchor is a member or a side-story
    /// anchor among the sides; the run is bounded; sections are disjoint
    /// from one another and from the run; every link joins members;
    /// asked again, the walk answers the same.
    #[test]
    fn the_answer_keeps_its_shape((cat, anchor) in catalogue()) {
        let (f, (known, details)) = drive_with(&anchor, &cat, &[]);
        let members = id_set(&f.entries);
        if members.contains(&anchor) {
            prop_assert!(f.parents.is_empty());
        } else {
            prop_assert!(id_set(&f.sides).contains(&anchor));
            prop_assert!(f.parents.contains_key(&anchor));
        }
        prop_assert!(members.len() <= MAX_MEMBERS);
        let sections = [&f.versions, &f.films, &f.sides, &f.recaps];
        let mut seen = members.clone();
        for s in sections {
            for e in s {
                prop_assert!(seen.insert(e.id.clone()), "{} twice", e.id);
            }
        }
        for (a, b) in &f.links {
            prop_assert!(members.contains(a) && members.contains(b));
        }
        for e in f.films.iter().chain(&f.sides).chain(&f.recaps) {
            prop_assert!(e.id == anchor || e.subtype.as_deref() != Some("music"));
        }
        prop_assert_eq!(plan(&anchor, &known, &details), Plan::Done(f.clone()));
        let everything = cat.listings.clone();
        prop_assert_eq!(plan(&anchor, &everything, &details), Plan::Done(f));
    }

    /// No member and an entry on its path list each other as another
    /// version or setting.
    #[test]
    fn no_member_is_an_alternative_of_its_path((cat, anchor) in catalogue()) {
        let (_, (known, details)) = drive_with(&anchor, &cat, &[]);
        let look = Lookup { known: &known, details: &details };
        let walked = run(&anchor, &look).expect("decided");
        for (m, via) in &walked.members {
            let mut at = via.clone();
            while let Some(p) = at {
                prop_assert!(!alt(&look, m, &p), "{} and {} on its path", m, p);
                at = walked.members[&p].clone();
            }
        }
    }
}
