//! The walk's bound on the listings it reads, held by the parent check
//! (`franchise_walk_parents.rs`) and the placing of a run of one-offs
//! (`franchise_walk_place.rs`) as by the walk's own steps: however many
//! candidate parents an anchor or its run names, the walk reads no more
//! listings than the bound, and an answer it cut short is incomplete.
//!
//! Every strategy returns a `BoxedStrategy`, for the Windows test
//! build's PDB limit (see `meta/kitsu_relations_prop_test.rs`).

use std::collections::BTreeMap;

use proptest::prelude::*;

use super::prop_tests::drive_with;
use super::rules::listed;
use super::tests::Catalogue;
use super::*;

/// A lone OVA that names ninety-eight earlier side stories reads no
/// more listings than the bound to check them, and says so.
#[test]
fn a_lone_side_story_checks_no_more_candidates_than_the_bound() {
    let mut c = Catalogue::default();
    c.add("1", "OVA", Some("2010-01-01"));
    for i in 0..98 {
        let id = format!("{}", 100 + i);
        c.add(&id, "TV", Some("2000-01-01"))
            .rel("1", "side_story", &id);
    }
    let (f, (known, _)) = drive_with("1", &c, &[]);
    assert!(known.len() <= MAX_LISTINGS, "read {} listings", known.len());
    assert!(!f.complete);
}

/// Two films in a run, each naming fifty earlier side stories: the
/// second film's check gets only what the first left of the bound.
#[test]
fn a_run_of_one_offs_shares_the_bound_between_its_members() {
    let mut c = Catalogue::default();
    c.add("1", "movie", Some("2010-01-01"))
        .add("2", "movie", Some("2012-01-01"))
        .rel("1", "sequel", "2")
        .rel("2", "prequel", "1");
    for (member, first) in [("1", 100), ("2", 200)] {
        for i in 0..50 {
            let id = format!("{}", first + i);
            c.add(&id, "TV", Some("2000-01-01"))
                .rel(member, "side_story", &id);
        }
    }
    let (f, (known, _)) = drive_with("1", &c, &[]);
    assert!(known.len() <= MAX_LISTINGS, "read {} listings", known.len());
    assert!(!f.complete);
}

/// A candidate: its role in the anchor's listing, its start year, and
/// whether it names the anchor back as its parent story.
type Candidate = (usize, u16, bool);

const ROLES: [&str; 3] = ["parent_story", "side_story", "spinoff"];
const FORMATS: [&str; 3] = ["OVA", "movie", "special"];

/// A lone anchor that is no series, naming up to a hundred and forty
/// candidate parents; every candidate's listing is complete.
fn hub() -> BoxedStrategy<Catalogue> {
    (
        0..FORMATS.len(),
        prop::collection::vec((0..ROLES.len(), 1990u16..2030, any::<bool>()), 0..140),
    )
        .prop_map(|(format, candidates): (usize, Vec<Candidate>)| {
            let mut c = Catalogue::default();
            c.add("1", FORMATS[format], Some("2010-01-01"));
            if let Some(e) = c.entries.get_mut("1") {
                e.episodes = Some(1);
            }
            for (i, (role, year, back)) in candidates.into_iter().enumerate() {
                let id = format!("{}", 100 + i);
                c.add(&id, "TV", Some(&format!("{year}-01-01")))
                    .rel("1", ROLES[role], &id);
                if back {
                    c.rel(&id, "parent_story", "1");
                }
            }
            c
        })
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// However many candidate parents the anchor names, the walk reads
    /// no more listings than the bound.
    #[test]
    fn the_parent_check_reads_no_more_than_the_bound(c in hub()) {
        let (_, (known, _)) = drive_with("1", &c, &[]);
        prop_assert!(known.len() <= MAX_LISTINGS, "read {} listings", known.len());
    }

    /// A complete answer read every candidate the anchor's parent check
    /// had to decide: what it names as its parent story, and the side
    /// stories and spin-offs it names that started before it.
    #[test]
    fn a_complete_answer_read_every_candidate(c in hub()) {
        let (f, (known, _)) = drive_with("1", &c, &[]);
        let details = BTreeMap::new();
        let look = Lookup { known: &c.listings, details: &details };
        let own = c.entries["1"].start.clone();
        let candidates: Vec<String> = listed(&look, "1", &["parent_story"])
            .chain(listed(&look, "1", &["side_story", "spinoff"]).filter(|r| {
                r.entry.as_ref().and_then(|e| e.start.clone()) < own
            }))
            .map(|r| r.id.clone())
            .collect();
        if f.complete {
            for id in &candidates {
                prop_assert!(known.contains_key(id), "complete, but {} unread", id);
            }
        }
    }
}
