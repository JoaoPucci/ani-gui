//! The first hop against the walk it foretells: named cases, generated
//! catalogues, and the corpus of real listings the study measured.
//!
//! Every strategy returns a `BoxedStrategy`, for the Windows test
//! build's PDB limit (see `meta/kitsu_relations_prop_test.rs`).

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;
use serde::Deserialize;

use super::super::tests::{drive, Catalogue};
use super::super::Franchise;
use super::{first_hop, FirstHop};
use crate::meta::kitsu::{FranchiseEntry, FranchiseListing};

/// What the show page draws from the walk's answer, as the frontend
/// reads it (`frontend/src/lib/watch-order/page-model.ts`): the strip
/// where the run holds the anchor and something beside it, or the
/// anchor is a side story with a parent story in the run
/// (`stripFor`); else the lone franchise link where the franchise page
/// would list anything but the anchor (`franchiseLinkOnly`); else
/// nothing.
fn drawn(f: &Franchise) -> FirstHop {
    let members: BTreeSet<&str> = f.entries.iter().map(|e| e.id.as_str()).collect();
    let anchor = f.anchor.as_str();
    let side_with_parent = !members.contains(anchor)
        && f.parents
            .get(anchor)
            .is_some_and(|ps| ps.iter().any(|p| members.contains(p.as_str())));
    if side_with_parent || (members.len() > 1 && members.contains(anchor)) {
        return FirstHop::Strip;
    }
    let rest = [&f.entries, &f.versions, &f.films, &f.sides, &f.recaps]
        .into_iter()
        .flatten()
        .any(|e| e.id != anchor);
    if rest {
        FirstHop::Link
    } else {
        FirstHop::None
    }
}

/// A catalogue with every entry added, from `(id, subtype)`.
fn with(entries: &[(&str, &str)]) -> Catalogue {
    let mut c = Catalogue::default();
    for (i, (id, subtype)) in entries.iter().enumerate() {
        let start = format!("{}-01-01", 2000 + i);
        c.add(id, subtype, Some(&start));
    }
    c
}

/// The first hop and the walk's drawn outcome, from `anchor`.
fn both(c: &Catalogue, anchor: &str) -> (FirstHop, FirstHop) {
    (
        first_hop(anchor, &c.listings[anchor]),
        drawn(&drive(anchor, c)),
    )
}

#[test]
fn a_sequel_foretells_the_strip() {
    let mut c = with(&[("1", "TV"), ("2", "TV")]);
    c.rel("1", "sequel", "2");
    assert_eq!(both(&c, "1"), (FirstHop::Strip, FirstHop::Strip));
}

#[test]
fn a_prequel_foretells_the_strip() {
    let mut c = with(&[("1", "TV"), ("2", "TV")]);
    c.rel("2", "prequel", "1");
    assert_eq!(both(&c, "2"), (FirstHop::Strip, FirstHop::Strip));
}

/// A side story whose own run is alone walks from its parent story and
/// gets that run's strip.
#[test]
fn a_parent_story_foretells_the_strip() {
    let mut c = with(&[("1", "TV"), ("2", "special"), ("3", "TV")]);
    c.rel("2", "parent_story", "1").rel("1", "sequel", "3");
    assert_eq!(both(&c, "2"), (FirstHop::Strip, FirstHop::Strip));
}

/// The franchise page lists a film, a side story, a recap or another
/// version beside a show with no run: the link comes alone.
#[test]
fn a_relation_around_the_show_foretells_the_link() {
    for role in ["side_story", "summary", "alternative_version", "spinoff"] {
        let mut c = with(&[("1", "TV"), ("2", "movie")]);
        c.rel("1", role, "2");
        assert_eq!(both(&c, "1"), (FirstHop::Link, FirstHop::Link), "{role}");
    }
}

/// A sequel the show itself lists as its other version is left out of
/// the run (the walk's rule 3) and lands among the versions.
#[test]
fn a_sequel_that_is_another_version_foretells_the_link() {
    let mut c = with(&[("1", "TV"), ("2", "TV")]);
    c.rel("1", "sequel", "2")
        .rel("1", "alternative_version", "2");
    assert_eq!(both(&c, "1"), (FirstHop::Link, FirstHop::Link));
}

/// Relations the franchise page never lists: a character crossover,
/// an adaptation, an alternative setting, a music video, a manga, and
/// the show itself.
#[test]
fn relations_the_franchise_page_never_lists_foretell_nothing() {
    let mut c = with(&[
        ("1", "TV"),
        ("2", "TV"),
        ("3", "TV"),
        ("4", "TV"),
        ("5", "music"),
    ]);
    c.rel("1", "character", "2")
        .rel("1", "adaptation", "3")
        .rel("1", "alternative_setting", "4")
        .rel("1", "side_story", "5")
        .rel("1", "sequel", "1");
    c.listings.get_mut("1").expect("listing").relations.push(
        crate::meta::kitsu::FranchiseRelation {
            role: "side_story".into(),
            kind: "manga".into(),
            id: "90".into(),
            subtype: Some("manga".into()),
            entry: None,
        },
    );
    assert_eq!(both(&c, "1"), (FirstHop::None, FirstHop::None));
}

#[test]
fn a_show_with_no_relations_foretells_nothing() {
    let c = with(&[("1", "TV")]);
    assert_eq!(both(&c, "1"), (FirstHop::None, FirstHop::None));
}

/// The case the anchor's listing cannot see: the sequel's own listing
/// names the show as its other version. The first hop says strip; the
/// walk leaves the sequel out and draws the link, and the page's
/// reserved strip gives way to it.
#[test]
fn a_version_named_only_from_the_other_side_is_the_one_miss() {
    let mut c = with(&[("1", "TV"), ("2", "TV")]);
    c.rel("1", "sequel", "2")
        .rel("2", "alternative_version", "1");
    assert_eq!(both(&c, "1"), (FirstHop::Strip, FirstHop::Link));
}

#[test]
fn the_wire_names_are_lowercase() {
    let names: Vec<String> = [FirstHop::Strip, FirstHop::Link, FirstHop::None]
        .iter()
        .map(|h| serde_json::to_string(h).expect("json"))
        .collect();
    assert_eq!(names, ["\"strip\"", "\"link\"", "\"none\""]);
}

// ---- generated catalogues ------------------------------------------------

const ROLES: [&str; 13] = [
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
    "adaptation",
    "spinoff",
    "other",
];
const SUBTYPES: [&str; 6] = ["TV", "movie", "OVA", "special", "music", "ONA"];
const ALTERNATIVES: [&str; 2] = ["alternative_version", "alternative_setting"];

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

/// A catalogue of `n` entries and an anchor among them. Each entry has
/// one subtype, so every listing that relates it agrees on it, as
/// Kitsu's do.
fn catalogue() -> BoxedStrategy<(Catalogue, String)> {
    (1usize..12)
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

/// `c` with every alternative relation dropped from the listings other
/// than `anchor`'s.
fn alternatives_only_from(mut c: Catalogue, anchor: &str) -> Catalogue {
    for (id, listing) in &mut c.listings {
        if id != anchor {
            listing
                .relations
                .retain(|r| !ALTERNATIVES.contains(&r.role.as_str()));
        }
    }
    c
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Where the first hop says the link or nothing, the walk agrees:
    /// a placeholder is never missing for a strip, and the link's
    /// placeholder is never wrong. Only a foretold strip can be wrong.
    #[test]
    fn only_a_foretold_strip_can_miss((c, anchor) in catalogue()) {
        let (hop, walked) = both(&c, &anchor);
        if hop != FirstHop::Strip {
            prop_assert_eq!(hop, walked);
        }
    }

    /// Where no listing but the anchor's names an alternative (the one
    /// thing the anchor's listing cannot see), the first hop is the
    /// walk's outcome.
    #[test]
    fn the_first_hop_is_the_walks_outcome((c, anchor) in catalogue()) {
        let c = alternatives_only_from(c, &anchor);
        let (hop, walked) = both(&c, &anchor);
        prop_assert_eq!(hop, walked);
    }
}

// ---- the corpus ----------------------------------------------------------

/// `tests/fixtures/kitsu/franchise_corpus.json`: the shows the study
/// measured, each anchor's entry as its detail read gives it, and every
/// listing their walks read.
#[derive(Deserialize)]
struct Corpus {
    anchors: Vec<String>,
    details: BTreeMap<String, FranchiseEntry>,
    listings: BTreeMap<String, FranchiseListing>,
}

fn corpus() -> Corpus {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/kitsu/franchise_corpus.json");
    let body = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_slice(&body).expect("corpus")
}

/// On every show of the corpus, the first hop is what the walk draws,
/// and the corpus holds each of the three outcomes.
#[test]
fn the_first_hop_matches_the_walk_on_the_corpus() {
    let corpus = corpus();
    let cat = Catalogue {
        entries: corpus.details,
        listings: corpus.listings,
    };
    let mut seen = BTreeMap::new();
    let mut wrong = Vec::new();
    for anchor in &corpus.anchors {
        let (hop, walked) = both(&cat, anchor);
        *seen.entry(format!("{walked:?}")).or_insert(0) += 1;
        if hop != walked {
            wrong.push((anchor.clone(), hop, walked));
        }
    }
    assert!(
        wrong.is_empty(),
        "first hop differs from the walk: {wrong:?}"
    );
    assert_eq!(corpus.anchors.len(), 120);
    assert_eq!(seen.len(), 3, "{seen:?}");
}
