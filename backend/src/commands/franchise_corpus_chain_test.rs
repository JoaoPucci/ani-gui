//! A page outside the main story walks its own chain: the franchise's
//! entries outside the main story joined to it by prequel and sequel
//! links, with the main-story titles where the chain leaves it or leads
//! into it (its seams), as `franchise_walk_chain.rs` cuts it. Pinned on
//! the corpus (`franchise_corpus_test.rs`), asked from every title of
//! each chain, and from main-story pages, which have none.

use std::collections::BTreeMap;

use super::corpus_page_tests::answers;
use super::corpus_tests::corpus;
use super::rules::alt;
use super::*;

/// A chain by id: its own titles and its seams.
type Ids = (Vec<String>, Vec<String>);

/// A page's chain as served, by id.
fn chain_of(f: &Franchise) -> Option<Ids> {
    let served = serde_json::to_value(f).expect("json");
    let chain = served.get("chain").filter(|c| !c.is_null())?;
    let ids = |key: &str| -> Vec<String> {
        chain[key]
            .as_array()
            .unwrap_or_else(|| panic!("chain.{key}"))
            .iter()
            .map(|v| {
                v.get("id")
                    .unwrap_or(v)
                    .as_str()
                    .expect("an id")
                    .to_string()
            })
            .collect()
    };
    let seams = ids("seams");
    let own = ids("entries")
        .into_iter()
        .filter(|id| !seams.contains(id))
        .collect();
    Some((own, seams))
}

fn list(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn a_page_outside_the_main_story_walks_its_own_chain() {
    let heavens_feel = (list(&["8675", "12007", "12008"]), list(&["7658"]));
    let golden_age = (list(&["6084", "6664", "6665"]), list(&["11655"]));
    let jojo_ovas = (list(&["610", "7730"]), list(&["7158"]));
    let want: BTreeMap<&str, Option<Ids>> = BTreeMap::from([
        // Heaven's Feel branches off after Fate/Zero 2nd Season.
        ("8675", Some(heavens_feel.clone())),
        ("12007", Some(heavens_feel.clone())),
        ("12008", Some(heavens_feel)),
        // The Unlimited Blade Works film, a chain of one.
        ("4788", Some((list(&["4788"]), list(&["7658"])))),
        // The Golden Age films lead into the 2016 series.
        ("6084", Some(golden_age.clone())),
        ("6664", Some(golden_age.clone())),
        ("6665", Some(golden_age)),
        // The OVAs follow the 2012 series by the 2000 OVA's own listing.
        ("7730", Some(jojo_ovas.clone())),
        ("610", Some(jojo_ovas)),
        // Boruto: Naruto the Movie branches off after Shippuden.
        ("10089", Some((list(&["10089"]), list(&["1555"])))),
        // Main-story pages have none.
        ("323", None),
        ("6028", None),
        ("7658", None),
        ("11655", None),
        ("24", None),
        ("7158", None),
        ("1555", None),
    ]);
    let answers: BTreeMap<String, Franchise> = answers().into_iter().collect();
    let mut failures = Vec::new();
    for (anchor, chain) in &want {
        let f = answers
            .get(*anchor)
            .unwrap_or_else(|| panic!("the corpus answers {anchor}"));
        let got = chain_of(f);
        if &got != chain {
            failures.push(format!("{anchor}: {got:?}, want {chain:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// No chain the corpus serves joins two entries that list each other as
/// versions, by a link between them or a seam: a version is never the
/// next title of the chain it retells. The corpus holds no listing that
/// names one entry both ways, so this pins every chain it serves as
/// clear of the case the worked cases in `franchise_walk_chain_test.rs`
/// build.
#[test]
fn no_chain_joins_two_versions_of_one_another() {
    let c = corpus();
    let look = Lookup {
        known: &c.listings,
        details: &c.details,
    };
    let mut failures = Vec::new();
    for (anchor, f) in answers() {
        let Some(chain) = &f.chain else { continue };
        for (a, b) in &chain.links {
            if alt(&look, a, b) {
                failures.push(format!("{anchor}: {a} -> {b}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
