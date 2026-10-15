//! The picker oracle's driver: runs the real pick walk over pools
//! the oracle under `tests/picker-oracle/` writes out, and over every
//! recorded fixture, and dumps each outcome in the one JSON shape the
//! oracle compares. The driver does not judge; it reports what the
//! walk did so the model of the docs, and the committed golden of the
//! recorded shows, can.
//!
//! Two of these tests are ignored by default: they take their input
//! and output paths from the environment and exist to be run by the
//! oracle's workflow or by hand. The golden comparison is an ordinary
//! test, so a change to a recorded show's pick fails `cargo test`
//! until the golden is regenerated — deliberately, in the same
//! change, with the reason in the commit body.

use super::*;

const SHOW: &str = r#"{"id": "one", "titles": ["Show"], "count": 12, "entry_year": 2020,
    "rows": [["x", "Show", 12]]}"#;

/// A pool parses from the oracle's shape; a listing is live unless
/// the case names it among the dead.
#[test]
fn a_case_parses_from_the_oracle_s_shape() {
    let case: Case = serde_json::from_str(SHOW).expect("parse");
    assert_eq!(case.id, "one");
    assert_eq!(case.titles, ["Show"]);
    assert_eq!((case.count, case.entry_year), (Some(12), Some(2020)));
    assert_eq!(case.rows, [("x".to_string(), "Show".to_string(), 12)]);
    assert!(case.dead.is_empty());
    let dead: Case = serde_json::from_str(
        r#"{"id": "d", "titles": ["Show"], "count": null, "entry_year": null,
            "rows": [["x", "Show", 12]], "dead": ["x"]}"#,
    )
    .expect("parse");
    assert_eq!(dead.dead, ["x"]);
}

/// A pick is reported as the slug, the regular count and every
/// episode's (id, number, tag); every listing a case writes numbers
/// its rows from 1 with no tag.
#[tokio::test]
async fn a_pick_s_outcome_names_the_slug_the_count_and_the_episodes() {
    let case: Case = serde_json::from_str(SHOW).expect("parse");
    let got = outcome(walk_over(site_of(&case)).await);
    let episodes: Vec<serde_json::Value> =
        (1..=12).map(|k| serde_json::json!([k, k, null])).collect();
    assert_eq!(
        got,
        serde_json::json!({"ok": "x", "count": 12, "episodes": episodes})
    );
}

/// A refusal is reported as the error's name and whether the pool was
/// the clean miss: a candidate the titles refuse is not one.
#[tokio::test]
async fn a_refusal_s_outcome_names_the_error_and_the_verdict() {
    let case: Case = serde_json::from_str(
        r#"{"id": "r", "titles": ["Show"], "count": 12, "entry_year": 2020,
            "rows": [["x", "Show 2", 12]]}"#,
    )
    .expect("parse");
    assert_eq!(
        outcome(walk_over(site_of(&case)).await),
        serde_json::json!({"err": "NoResults", "clean_miss": false})
    );
    let empty: Case = serde_json::from_str(
        r#"{"id": "e", "titles": ["Show"], "count": 12, "entry_year": 2020, "rows": []}"#,
    )
    .expect("parse");
    assert_eq!(
        outcome(walk_over(site_of(&empty)).await),
        serde_json::json!({"err": "NoResults", "clean_miss": true})
    );
}

/// A listing the case names among the dead dies in transport when
/// probed, as the fixtures' dead listings do.
#[tokio::test]
async fn a_dead_listing_dies_when_probed() {
    let case: Case = serde_json::from_str(
        r#"{"id": "d", "titles": ["Show"], "count": 12, "entry_year": 2020,
            "rows": [["x", "Show", 12]], "dead": ["x"]}"#,
    )
    .expect("parse");
    let got = outcome(walk_over(site_of(&case)).await);
    assert!(got.get("err").is_some(), "{got}");
    assert_eq!(got["clean_miss"], false, "{got}");
}

/// The dump is one fixture per line, in id order, so a diff of two
/// dumps names the shows whose picks changed and nothing else.
#[test]
fn the_dump_renders_one_fixture_per_line_in_id_order() {
    let dump: BTreeMap<String, serde_json::Value> = [
        (
            "7".to_string(),
            serde_json::json!({"ok": "b", "count": 1, "episodes": [[1, 1, null]]}),
        ),
        (
            "11".to_string(),
            serde_json::json!({"err": "NoResults", "clean_miss": true}),
        ),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        render(&dump),
        "{\n  \"11\": {\"clean_miss\":true,\"err\":\"NoResults\"},\n  \"7\": {\"count\":1,\"episodes\":[[1,1,null]],\"ok\":\"b\"}\n}\n"
    );
    let parsed: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(&render(&dump)).expect("the dump is JSON");
    assert_eq!(parsed, dump);
}

/// Every recorded fixture picks as the committed golden says it does.
/// A pick that changes here is a pick that changes for that show: the
/// change regenerates the golden and says why, or it is a regression.
#[tokio::test]
async fn recorded_fixtures_pick_as_the_golden_dump_records() {
    let golden: BTreeMap<String, serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(golden_path())
            .unwrap_or_else(|e| panic!("read {}: {e}", golden_path().display())),
    )
    .expect("parse the golden dump");
    let now = dump_fixtures().await;
    let mut wrong = Vec::new();
    for id in golden
        .keys()
        .chain(now.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        if golden.get(id) != now.get(id) {
            wrong.push(format!(
                "  {id}:\n    golden {}\n    now    {}",
                golden
                    .get(id)
                    .map_or("(absent)".to_string(), |v| v.to_string()),
                now.get(id)
                    .map_or("(absent)".to_string(), |v| v.to_string()),
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "the recorded fixtures no longer pick as {} records:\n{}\n\
         A recorded show's pick changed. When that is the change's intent, regenerate the golden \
         in the same change — from backend/, `cargo test --lib write_the_fixture_golden -- --ignored` \
         — and say why in the commit body. Otherwise it is a regression.",
        golden_path().display(),
        wrong.join("\n")
    );
}
