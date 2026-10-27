//! The episode-map oracle's driver: maps every case the model under
//! `tests/tracker-map-oracle/` writes out and reports, per case, the map
//! and every Kitsu episode's write, in the one JSON shape the model
//! compares. The driver does not judge; `oracle.py cmp` does.

use super::*;
use crate::account::episode_map::{EpisodeMap, Evidence, Target};

/// The most episodes a case's writes are listed for — the model's
/// `TARGET_CAP`.
const TARGET_CAP: u32 = 300;

#[derive(serde::Deserialize)]
struct Case {
    id: String,
    evidence: Evidence,
}

/// Every Kitsu episode's write under `map`, 1 to `count + 1` (one past
/// the end, which is outside every segment): `null` for identity, else
/// `[entry, progress, complete, [[earlier entry, total, finished], …]]`.
fn targets(map: &EpisodeMap, count: u32) -> serde_json::Value {
    let count = count.min(TARGET_CAP);
    (1..=count + 1)
        .map(|k| match map.target(k) {
            Target::Identity => serde_json::Value::Null,
            Target::Mapped(w) => serde_json::json!([
                w.entry,
                w.progress,
                w.complete,
                w.earlier
                    .iter()
                    .map(|e| serde_json::json!([e.entry, e.total, e.finished]))
                    .collect::<Vec<_>>()
            ]),
        })
        .collect()
}

fn outcome(evidence: &Evidence) -> serde_json::Value {
    let map = map_show(evidence);
    serde_json::json!({
        "map": map,
        "targets": targets(&map, evidence.kitsu.episodes.unwrap_or(0)),
    })
}

/// Maps every case the file `TRACKER_MAP_ORACLE_CASES` holds and writes
/// each outcome to `TRACKER_MAP_ORACLE_OUT` as `[{"id": …, "got": …}]`.
#[test]
#[ignore = "driven by tests/tracker-map-oracle: reads TRACKER_MAP_ORACLE_CASES, writes TRACKER_MAP_ORACLE_OUT"]
fn run_the_tracker_map_oracle_cases() {
    let input = std::env::var("TRACKER_MAP_ORACLE_CASES")
        .expect("TRACKER_MAP_ORACLE_CASES names the case file");
    let output = std::env::var("TRACKER_MAP_ORACLE_OUT")
        .expect("TRACKER_MAP_ORACLE_OUT names the outcome file");
    let cases: Vec<Case> = serde_json::from_str(
        &std::fs::read_to_string(&input).unwrap_or_else(|e| panic!("read {input}: {e}")),
    )
    .unwrap_or_else(|e| panic!("parse {input}: {e}"));
    let out: Vec<serde_json::Value> = cases
        .iter()
        .map(|c| serde_json::json!({"id": c.id, "got": outcome(&c.evidence)}))
        .collect();
    std::fs::write(&output, serde_json::to_string(&out).expect("serialise"))
        .unwrap_or_else(|e| panic!("write {output}: {e}"));
}

#[test]
fn an_outcome_lists_the_map_and_every_episode_s_write() {
    let case: Case = serde_json::from_str(
        r#"{"id": "x", "evidence": {
            "kitsu": {"id": "186", "episodes": 3, "start": "1989-04-15", "end": "1989-09-25"},
            "entry": {"id": 210, "episodes": 1, "finished": true, "start": "1989-04-15", "end": "1989-04-20"},
            "chain": [{"id": 9, "episodes": 2, "finished": false, "start": "1989-05-01", "end": "1989-09-25"}]}}"#,
    )
    .expect("parse");
    let got = outcome(&case.evidence);
    assert_eq!(got["map"]["shape"], "chain");
    assert_eq!(
        got["targets"],
        serde_json::json!([
            [210, 1, true, []],
            [9, 1, false, [[210, 1, true]]],
            [9, 2, false, [[210, 1, true]]],
            null
        ])
    );
    let unknown: Case = serde_json::from_str(
        r#"{"id": "y", "evidence": {"kitsu": {"id": "1", "episodes": null}, "entry": {"id": 2, "episodes": 5, "finished": true}}}"#,
    )
    .expect("parse");
    let got = outcome(&unknown.evidence);
    assert_eq!(got["map"]["why"], "count_unknown");
    assert_eq!(got["targets"], serde_json::json!([null]));
}

#[test]
fn a_long_show_s_writes_stop_at_the_cap() {
    let ev: Evidence = serde_json::from_str(
        r#"{"kitsu": {"id": "1", "episodes": 500}, "entry": {"id": 2, "episodes": 500, "finished": true}}"#,
    )
    .expect("parse");
    let got = outcome(&ev);
    assert_eq!(got["targets"].as_array().map(Vec::len), Some(301));
}
