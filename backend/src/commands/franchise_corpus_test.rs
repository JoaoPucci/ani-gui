//! The franchise corpus: the walk run over the listings Kitsu served
//! for big franchises (`tests/fixtures/franchise/listings.json`, as
//! `franchise_corpus_record_test.rs` recorded them), from every anchor
//! `cases.json` names, against what the strip and the franchise page
//! must say for it: the main story's first entry (the franchise page's
//! title), whether the anchor is in the main story or around it, and a
//! side story's parent stories.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::*;
use crate::meta::kitsu::{FranchiseEntry, FranchiseListing};

/// The recorded listings and details.
#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct Corpus {
    pub(super) listings: BTreeMap<String, FranchiseListing>,
    pub(super) details: BTreeMap<String, FranchiseEntry>,
}

/// The corpus as JSON, one listing or detail per line.
pub(super) fn to_lines(c: &Corpus) -> String {
    fn block<T: Serialize>(name: &str, map: &BTreeMap<String, T>) -> String {
        let rows: Vec<String> = map
            .iter()
            .map(|(k, v)| {
                format!(
                    "    {}: {}",
                    serde_json::to_string(k).expect("key"),
                    serde_json::to_string(v).expect("value")
                )
            })
            .collect();
        format!("  \"{name}\": {{\n{}\n  }}", rows.join(",\n"))
    }
    format!(
        "{{\n{},\n{}\n}}\n",
        block("listings", &c.listings),
        block("details", &c.details)
    )
}

/// What one anchor must read as.
#[derive(Debug, Deserialize)]
struct Case {
    franchise: String,
    anchor: String,
    /// The title of the main story's first entry.
    first: String,
    /// Where the anchor sits: `entries`, `sides`, `films`, `versions`
    /// or `recaps`.
    anchor_in: String,
    /// A side story's parent stories, by id.
    #[serde(default)]
    parents: Vec<String>,
    /// Ids that must sit in a section: `{section: [ids]}`.
    #[serde(default)]
    sections: BTreeMap<String, Vec<String>>,
    // `note`, what the case is about, is for the reader.
}

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/franchise")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

pub(super) fn corpus() -> Corpus {
    serde_json::from_slice(&fixture("listings.json")).expect("listings.json")
}

/// The walk over the corpus from `anchor`.
pub(super) fn walk(c: &Corpus, anchor: &str) -> Franchise {
    match plan(anchor, &c.listings, &c.details) {
        Plan::Done(f) => f,
        Plan::Need { listings, details } => {
            panic!("{anchor}: the corpus lacks listings {listings:?} / details {details:?}")
        }
    }
}

fn section<'a>(f: &'a Franchise, name: &str) -> &'a [FranchiseEntry] {
    match name {
        "entries" => &f.entries,
        "sides" => &f.sides,
        "films" => &f.films,
        "versions" => &f.versions,
        "recaps" => &f.recaps,
        other => panic!("no section {other}"),
    }
}

/// What a case finds wrong with `f`, as lines; empty when it reads as
/// pinned.
fn wrong(case: &Case, f: &Franchise) -> Vec<String> {
    let mut out = Vec::new();
    let first = f.entries.first().map(|e| e.title.as_str());
    if first != Some(case.first.as_str()) {
        out.push(format!("first entry {first:?}, want {:?}", case.first));
    }
    if !section(f, &case.anchor_in)
        .iter()
        .any(|e| e.id == case.anchor)
    {
        out.push(format!("anchor not in {}", case.anchor_in));
    }
    let parents = f.parents.get(&case.anchor).cloned().unwrap_or_default();
    if parents != case.parents {
        out.push(format!("parents {parents:?}, want {:?}", case.parents));
    }
    for e in f.versions.iter().chain(&f.sides).chain(&f.recaps) {
        if e.subtype.as_deref() == Some("movie") {
            out.push(format!(
                "the film {} ({}) is not under films",
                e.id, e.title
            ));
        }
    }
    for (name, ids) in &case.sections {
        for id in ids {
            if !section(f, name).iter().any(|e| &e.id == id) {
                out.push(format!("{id} not in {name}"));
            }
        }
    }
    out
}

#[test]
fn every_corpus_anchor_reads_as_pinned() {
    let c = corpus();
    let cases: Vec<Case> = serde_json::from_slice(&fixture("cases.json")).expect("cases.json");
    let failures: Vec<String> = cases
        .iter()
        .flat_map(|case| {
            let f = walk(&c, &case.anchor);
            wrong(case, &f)
                .into_iter()
                .map(move |w| format!("{} ({}): {w}", case.franchise, case.anchor))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The relations of Kitsu's pages `names` (under
/// `tests/fixtures/kitsu/`), artwork dropped as the corpus drops it.
fn served(names: &[&str]) -> Vec<crate::meta::kitsu::FranchiseRelation> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/kitsu");
    names
        .iter()
        .flat_map(|n| {
            let body = std::fs::read(root.join(n)).expect("page");
            crate::meta::kitsu::parse_franchise_page(&body)
                .expect("parsed")
                .relations
        })
        .map(|mut r| {
            r.entry = r.entry.map(|mut e| {
                e.poster = None;
                e.cover = None;
                e
            });
            r
        })
        .collect()
}

#[test]
fn the_corpus_holds_what_kitsu_served_for_one_piece_and_lego_one_piece() {
    let c = corpus();
    let one_piece = served(&[
        "franchise_rel_12_o0.json",
        "franchise_rel_12_o20.json",
        "franchise_rel_12_o40.json",
        "franchise_rel_12_o60.json",
        "franchise_rel_12_o80.json",
    ]);
    assert_eq!(c.listings["12"].relations, one_piece);
    assert_eq!(
        c.listings["50656"].relations,
        served(&["franchise_rel_50656.json"])
    );
    // The pair as Kitsu serves it, from both sides: the series names the
    // two-episode ONA its parent story, and the ONA names the series
    // its side story.
    let role = |of: &str, id: &str| {
        c.listings[of]
            .relations
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.role.clone())
    };
    assert_eq!(role("12", "50656").as_deref(), Some("parent_story"));
    assert_eq!(role("50656", "12").as_deref(), Some("side_story"));
}
