//! Recording the franchise corpus (`tests/fixtures/franchise/`): the
//! walk run against live Kitsu from every anchor `cases.json` names,
//! every listing and detail it asked for kept in `listings.json`. The
//! test is ignored and run by hand; it writes the fixture and asserts
//! nothing about the walk.
//!
//! A listing or detail already in `listings.json` is answered from it,
//! so a recording can be re-run after the walk changes and asks only
//! for what it did not read before. A live request waits at least
//! [`PACING`] after the one before it. Artwork is dropped: the walk
//! never reads it.
//!
//! ```sh
//! cd "$ROOT/backend"
//! cargo test --lib record_the_franchise_corpus -- --ignored
//! ```

use std::path::PathBuf;

use super::*;
use crate::commands::franchise::entry_from_detail;
use crate::meta::kitsu::{FranchiseEntry, FranchiseListing, KitsuClient};

/// The least time between two live requests.
const PACING: std::time::Duration = std::time::Duration::from_millis(700);

fn dir() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/franchise")
}

fn bare(mut e: FranchiseEntry) -> FranchiseEntry {
    e.poster = None;
    e.cover = None;
    e
}

#[tokio::test]
#[ignore = "records the franchise corpus from live Kitsu"]
async fn record_the_franchise_corpus() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(dir().join("cases.json")).expect("cases"))
            .expect("cases json");
    let path = dir().join("listings.json");
    let mut corpus: super::corpus_tests::Corpus = std::fs::read(&path)
        .ok()
        .map(|b| serde_json::from_slice(&b).expect("corpus json"))
        .unwrap_or_default();
    let client = KitsuClient::new(reqwest::Client::new());
    let mut last = std::time::Instant::now() - PACING;
    let mut pace = || {
        let wait = PACING.saturating_sub(last.elapsed());
        std::thread::sleep(wait);
        last = std::time::Instant::now();
    };
    for case in cases {
        let anchor = case["anchor"].as_str().expect("anchor").to_string();
        loop {
            let need = plan(&anchor, &corpus.listings, &corpus.details);
            let (listings, details) = match need {
                Plan::Done(_) => break,
                Plan::Need { listings, details } => (listings, details),
            };
            for id in listings {
                pace();
                let mut l: FranchiseListing =
                    client.franchise_relations(&id).await.expect("listing");
                for r in &mut l.relations {
                    r.entry = r.entry.take().map(bare);
                }
                corpus.listings.insert(id, l);
            }
            for id in details {
                pace();
                let d = client.anime_detail(&id).await.expect("detail");
                corpus.details.insert(id, bare(entry_from_detail(&d)));
            }
        }
        std::fs::write(&path, super::corpus_tests::to_lines(&corpus)).expect("write");
    }
}
