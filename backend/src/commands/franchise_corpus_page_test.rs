//! The franchise page is the franchise's: every page of a franchise in
//! the corpus (`franchise_corpus_test.rs`) reads the same one, whichever
//! of its pages asks. Every listing the corpus holds is asked from, its
//! entry's details taken from a listing that relates it where the
//! corpus kept none.

use std::collections::BTreeMap;

use super::corpus_tests::corpus;
use super::*;
use crate::meta::kitsu::FranchiseEntry;

/// What the franchise page shows: everything but which page asked, and
/// that page's own parents and chain.
fn page(f: &Franchise) -> serde_json::Value {
    let mut v = serde_json::to_value(f).expect("json");
    let map = v.as_object_mut().expect("object");
    for own in ["anchor", "parents", "chain"] {
        map.remove(own);
    }
    v
}

/// The answer from every listing the corpus holds, where the corpus
/// holds all the walk reads.
fn answers() -> Vec<(String, Franchise)> {
    let mut c = corpus();
    let ids: Vec<String> = c.listings.keys().cloned().collect();
    for id in &ids {
        if c.details.contains_key(id) {
            continue;
        }
        let found: Option<FranchiseEntry> = c
            .listings
            .values()
            .flat_map(|l| &l.relations)
            .find(|r| &r.id == id && r.entry.is_some())
            .and_then(|r| r.entry.clone());
        if let Some(e) = found {
            c.details.insert(id.clone(), e);
        }
    }
    ids.into_iter()
        .filter_map(|id| match plan(&id, &c.listings, &c.details) {
            Plan::Done(f) => Some((id, f)),
            Plan::Need { .. } => None,
        })
        .collect()
}

#[test]
fn every_page_of_a_franchise_reads_one_franchise_page() {
    let answers = answers();
    let mut failures = BTreeMap::new();
    for (i, (a, f)) in answers.iter().enumerate() {
        for (b, g) in &answers[i + 1..] {
            let shared = f
                .entries
                .iter()
                .any(|e| g.entries.iter().any(|x| x.id == e.id));
            if shared && page(f) != page(g) {
                failures
                    .entry(f.name.clone())
                    .or_insert_with(Vec::new)
                    .push(format!("{a} and {b}"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
