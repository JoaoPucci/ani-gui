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

/// One pool as the oracle writes it: an entry known by `titles` with
/// `count` and `entry_year`, every title's search answering with the
/// rows `(slug, title, episodes)`. Every listing numbers its rows from
/// 1 with no tag and its page names [`LISTING_YEAR`]; `dead` names the
/// slugs whose probe dies in transport.
#[derive(Debug, serde::Deserialize)]
struct Case {
    id: String,
    titles: Vec<String>,
    count: Option<u32>,
    entry_year: Option<u32>,
    rows: Vec<(String, String, u32)>,
    #[serde(default)]
    dead: Vec<String>,
}

/// The year every listing a case writes names on its page — the
/// entry's own year wherever the case gives it one, so the year
/// vouches for every hit and the titles alone decide.
const LISTING_YEAR: u32 = 2020;

/// The site as one case records it.
fn site_of(case: &Case) -> Recorded {
    let canonical = case
        .titles
        .first()
        .unwrap_or_else(|| panic!("case {}: no titles", case.id));
    let listing = |n: u32| {
        serde_json::Value::Array((1..=n).map(|k| serde_json::json!([k, k, null])).collect())
    };
    let hits: Vec<serde_json::Value> = case
        .rows
        .iter()
        .map(|(slug, title, _)| serde_json::json!([slug, title, "TV"]))
        .collect();
    let json = serde_json::json!({
        "kitsu": {
            "canonical": canonical,
            "alt_titles": case.titles[1..],
            "episode_count": case.count,
            "year": case.entry_year,
            "subtype": "TV",
        },
        "searches": case.titles.iter().map(|t| (t.clone(), serde_json::Value::Array(hits.clone()))).collect::<serde_json::Map<_, _>>(),
        "episodes": case.rows.iter().map(|(slug, _, n)| (slug.clone(), listing(*n))).collect::<serde_json::Map<_, _>>(),
        "years": case.rows.iter().map(|(slug, _, _)| (slug.clone(), serde_json::json!(LISTING_YEAR))).collect::<serde_json::Map<_, _>>(),
    });
    let mut site = Recorded::inline(&case.id, &json.to_string());
    // The recording names its dead listings as static strings, as the
    // fixture tests write them in source. A case's arrive at run time,
    // and the driver runs each case once per process, so leaking them
    // is the whole cost.
    site.dead = case
        .dead
        .iter()
        .map(|slug| -> &'static str { slug.clone().leak() })
        .collect();
    site
}

/// What the walk did, in the shape the oracle compares: a pick as its
/// slug, regular count and every episode's `(id, number, tag)`; a
/// refusal as the error's name and whether the pool was the clean miss.
fn outcome(result: std::result::Result<PickedShow, NativeError>) -> serde_json::Value {
    match result {
        Ok(p) => {
            let count = crate::commands::play_native_numbering::regular_episode_count(&p.episodes);
            let episodes: Vec<serde_json::Value> = p
                .episodes
                .iter()
                .map(|e| serde_json::json!([e.id, e.number, e.number2]))
                .collect();
            serde_json::json!({"ok": p.hit.slug, "count": count, "episodes": episodes})
        }
        Err(e) => serde_json::json!({"err": format!("{:?}", e.error), "clean_miss": e.clean_miss}),
    }
}

/// The committed dump of every recorded fixture's outcome.
fn golden_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/picker-oracle/fixtures-golden.json")
}

/// Every recorded fixture's outcome, by id. A fixture whose walk
/// panics is reported as such rather than ending the dump.
async fn dump_fixtures() -> BTreeMap<String, serde_json::Value> {
    let ids = std::fs::read_dir(fixture_dir())
        .expect("list fixtures")
        .filter_map(|entry| {
            let name = entry.expect("dir entry").file_name();
            let id = name.to_string_lossy().strip_suffix(".json")?.to_string();
            (!id.is_empty() && id.chars().all(|c| c.is_ascii_digit())).then_some(id)
        });
    let mut out = BTreeMap::new();
    for id in ids {
        let run = id.clone();
        let got = tokio::spawn(async move { outcome(walk(&run).await) })
            .await
            .unwrap_or_else(|_| serde_json::json!({"panic": true}));
        out.insert(id, got);
    }
    out
}

/// The dump as text: one fixture per line, in id order, so a diff
/// between two names the shows whose picks changed and nothing else.
fn render(dump: &BTreeMap<String, serde_json::Value>) -> String {
    let mut out = String::from("{\n");
    let last = dump.len().saturating_sub(1);
    for (i, (id, got)) in dump.iter().enumerate() {
        let comma = if i < last { "," } else { "" };
        out.push_str(&format!(
            "  {}: {got}{comma}\n",
            serde_json::Value::String(id.clone())
        ));
    }
    out.push_str("}\n");
    out
}

/// Runs every pool the file `PICKER_ORACLE_CASES` holds and writes
/// each outcome to `PICKER_ORACLE_OUT` as `[{"id": …, "got": …}, …]`,
/// for the oracle's `cmp` to read against its predictions.
#[tokio::test]
#[ignore = "driven by tests/picker-oracle: reads PICKER_ORACLE_CASES, writes PICKER_ORACLE_OUT"]
async fn run_the_oracle_cases() {
    let input =
        std::env::var("PICKER_ORACLE_CASES").expect("PICKER_ORACLE_CASES names the case file");
    let output =
        std::env::var("PICKER_ORACLE_OUT").expect("PICKER_ORACLE_OUT names the outcome file");
    let cases: Vec<Case> = serde_json::from_str(
        &std::fs::read_to_string(&input).unwrap_or_else(|e| panic!("read {input}: {e}")),
    )
    .unwrap_or_else(|e| panic!("parse {input}: {e}"));
    let mut out = Vec::with_capacity(cases.len());
    for case in cases {
        let id = case.id.clone();
        let got = tokio::spawn(async move { outcome(walk_over(site_of(&case)).await) })
            .await
            .unwrap_or_else(|_| serde_json::json!({"panic": true}));
        out.push(serde_json::json!({"id": id, "got": got}));
    }
    std::fs::write(&output, serde_json::to_string(&out).expect("serialise"))
        .unwrap_or_else(|e| panic!("write {output}: {e}"));
}

/// Rewrites the golden from the fixtures as they pick now. Run it in
/// the change that means to alter a recorded show's pick, and say why
/// in that commit's body.
#[tokio::test]
#[ignore = "rewrites tests/picker-oracle/fixtures-golden.json"]
async fn write_the_fixture_golden() {
    std::fs::write(golden_path(), render(&dump_fixtures().await))
        .unwrap_or_else(|e| panic!("write {}: {e}", golden_path().display()));
}

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
