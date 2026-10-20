//! Every recorded tracker mismatch, mapped: the cases under
//! `tests/fixtures/tracker-map/` (one per Kitsu entry whose count a
//! tracker's entry disagrees with, found by scanning Kitsu's whole
//! catalogue against AniList and MyAnimeList) each name the shape the
//! documented rules answer, and every outcome is compared with the
//! committed dump `tests/tracker-map-oracle/fixtures-golden.json`, so a
//! change that moves any recorded show's map fails `cargo test` until
//! the golden is regenerated in the same change, with the reason.

use std::collections::BTreeMap;

use super::*;
use crate::account::episode_map::{EpisodeMap, Evidence, Shape};

#[derive(serde::Deserialize)]
struct Case {
    id: String,
    evidence: Evidence,
    expect: String,
}

#[derive(serde::Deserialize)]
struct Cases {
    cases: Vec<Case>,
}

fn repo() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn recorded() -> Vec<Case> {
    let mut all = Vec::new();
    for tracker in ["anilist", "mal"] {
        let path = repo().join(format!("tests/fixtures/tracker-map/{tracker}.json"));
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let cases: Cases =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
        all.extend(cases.cases);
    }
    all
}

/// The name a case's `expect` gives a shape.
fn shape_name(shape: &Shape) -> &'static str {
    match shape {
        Shape::Identity(_) => "identity",
        Shape::Chain => "chain",
        Shape::SharedEntry => "shared_entry",
        Shape::SplitParts => "split_parts",
        Shape::Extras => "extras",
    }
}

fn golden_path() -> std::path::PathBuf {
    repo().join("tests/tracker-map-oracle/fixtures-golden.json")
}

/// Every recorded case's map, by id, one per line.
fn render(dump: &BTreeMap<String, EpisodeMap>) -> String {
    let mut out = String::from("{\n");
    let last = dump.len().saturating_sub(1);
    for (i, (id, map)) in dump.iter().enumerate() {
        let comma = if i < last { "," } else { "" };
        let map = serde_json::to_string(map).expect("serialise");
        out.push_str(&format!(
            "  {}: {map}{comma}\n",
            serde_json::Value::String(id.clone())
        ));
    }
    out.push_str("}\n");
    out
}

fn dump() -> BTreeMap<String, EpisodeMap> {
    recorded()
        .into_iter()
        .map(|c| {
            let map = map_show(&c.evidence);
            (c.id, map)
        })
        .collect()
}

#[test]
fn every_recorded_mismatch_maps_to_the_shape_its_case_names() {
    let wrong: Vec<String> = recorded()
        .iter()
        .filter_map(|c| {
            let got = map_show(&c.evidence);
            (shape_name(&got.shape) != c.expect)
                .then(|| format!("{}: expected {}, got {:?}", c.id, c.expect, got.shape))
        })
        .collect();
    assert!(
        wrong.is_empty(),
        "{} cases:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

#[test]
fn recorded_tracker_fixtures_map_as_the_golden_dump_records() {
    let golden = std::fs::read_to_string(golden_path())
        .unwrap_or_else(|e| panic!("read {}: {e}", golden_path().display()));
    let now = render(&dump());
    if golden != now {
        let old: BTreeMap<String, serde_json::Value> =
            serde_json::from_str(&golden).expect("golden parses");
        let new: BTreeMap<String, serde_json::Value> =
            serde_json::from_str(&now).expect("dump parses");
        let moved: Vec<&String> = old
            .keys()
            .chain(new.keys())
            .filter(|k| old.get(*k) != new.get(*k))
            .collect();
        panic!(
            "recorded maps moved ({}): {moved:?}\nregenerate with `cargo test --lib write_the_tracker_fixture_golden -- --ignored` in a change that means it",
            moved.len()
        );
    }
}

/// Rewrites the golden from the recorded cases as they map now. Run it
/// in the change that means to alter a recorded show's map, and say
/// why in that commit's body.
#[test]
#[ignore = "rewrites tests/tracker-map-oracle/fixtures-golden.json"]
fn write_the_tracker_fixture_golden() {
    std::fs::write(golden_path(), render(&dump()))
        .unwrap_or_else(|e| panic!("write {}: {e}", golden_path().display()));
}

#[test]
fn the_dump_is_one_case_per_line_in_id_order() {
    let mut d = BTreeMap::new();
    d.insert(
        "b".to_string(),
        EpisodeMap::identity(crate::account::episode_map::Identity::CountsAgree),
    );
    d.insert(
        "a".to_string(),
        EpisodeMap::identity(crate::account::episode_map::Identity::Conflict),
    );
    assert_eq!(
        render(&d),
        "{\n  \"a\": {\"shape\":\"identity\",\"why\":\"conflict\",\"segments\":[]},\n  \"b\": {\"shape\":\"identity\",\"why\":\"counts_agree\",\"segments\":[]}\n}\n"
    );
    assert_eq!(render(&BTreeMap::new()), "{\n}\n");
}
