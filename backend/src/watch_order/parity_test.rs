//! The parity record (`tests/fixtures/watch-order/parity.json`): every
//! decision these modules make, over the fixture franchises, the walk's
//! served corpus (`tests/fixtures/franchise/served-*.json`) and a fixed
//! set of generated payloads, each with the inputs it was made from. The
//! renderer's watch-order modules were held to the same file while they
//! lived (their `parity.test.ts`), which is how the port showed it
//! decides as they did.
//!
//! Entries in the answers are written as their ids: every entry an
//! answer names is one of its input's, so its id says which.
//!
//! ```sh
//! cd "$ROOT/backend"
//! cargo test --lib write_the_parity_record -- --ignored
//! ```

use std::collections::BTreeSet;

use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::TestRunner;
use serde::Serialize;
use serde_json::{json, Value};

use super::chain_end::{player_up_next, Finale};
use super::fixtures::{dir, payload, runs};
use super::franchise_link::franchise_link_only;
use super::franchise_page::franchise_page;
use super::handoff::{handoff_ask, handoff_from, AskRow, HandoffMatch};
use super::page_model::watch_order_page;
use super::strategies::{franchise_arb, franchise_around_arb, rows_arb};
use super::{EntryProgress, Franchise, HistoryRow, WatchOrder};

const TODAY: &str = "2026-11-02";
/// How many generated payloads the record holds.
const GENERATED: usize = 300;

/// One input: a payload answered for its anchor, the rows its entries
/// resume from, and the ids the hand-off counts as played.
#[derive(Debug, Clone)]
pub(crate) struct Input {
    name: String,
    /// Where the payload comes from, for the record: a fixture run and
    /// its anchor, a served file, or the payload itself.
    source: Value,
    franchise: Franchise,
    rows: Vec<HistoryRow>,
    played: BTreeSet<String>,
}

impl Input {
    fn recorded(&self) -> Value {
        json!({ "name": self.name, "source": self.source, "rows": self.rows, "played": self.played })
    }
}

/// The fixture runs answered for each title they list, with their
/// made-up progress as rows.
fn fixture_inputs() -> Vec<Input> {
    let mut out = Vec::new();
    for r in runs() {
        let rows: Vec<HistoryRow> = r
            .progress
            .iter()
            .map(|(id, p)| HistoryRow {
                kitsu_id: Some(id.clone()),
                ep_no: match p {
                    EntryProgress::Watched => "999".into(),
                    EntryProgress::Episode(n) => n.to_string(),
                },
            })
            .collect();
        for e in r.run.all() {
            out.push(Input {
                name: format!("fixture {} {}", r.key, e.id),
                source: json!({ "fixture": r.key, "anchor": e.id }),
                franchise: payload(&r.key, &e.id),
                rows: rows.clone(),
                played: BTreeSet::new(),
            });
        }
    }
    out
}

/// The walk's answers the frontend's tests are served.
fn served_inputs() -> Vec<Input> {
    let folder = dir().parent().expect("fixtures").join("franchise");
    let mut paths: Vec<_> = std::fs::read_dir(&folder)
        .expect("the served corpus")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("served-"))
        })
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let franchise: Franchise =
                serde_json::from_slice(&std::fs::read(&p).expect("a served file"))
                    .expect("served json");
            Input {
                name: format!("served {}", franchise.anchor),
                source: json!({ "served": franchise.anchor }),
                rows: Vec::new(),
                played: BTreeSet::new(),
                franchise,
            }
        })
        .collect()
}

/// A fixed set of generated payloads, any shape the parser could hand over.
fn generated_inputs() -> Vec<Input> {
    let mut runner = TestRunner::deterministic();
    let strategy =
        (proptest::prop_oneof![franchise_arb(), franchise_around_arb()]).prop_flat_map(|f| {
            let ids: Vec<String> = f.entries.iter().map(|e| e.id.clone()).collect();
            let played = proptest::sample::subsequence(ids.clone(), 0..=ids.len().min(2));
            (proptest::strategy::Just(f), rows_arb(ids), played)
        });
    (0..GENERATED)
        .map(|i| {
            let (franchise, rows, played) =
                strategy.new_tree(&mut runner).expect("a value").current();
            Input {
                name: format!("generated {i}"),
                source: json!({ "payload": franchise }),
                franchise,
                rows,
                played: played.into_iter().collect(),
            }
        })
        .collect()
}

pub(crate) fn inputs() -> Vec<Input> {
    let mut all = fixture_inputs();
    all.extend(served_inputs());
    all.extend(generated_inputs());
    all
}

/// `v` with every entry written as its id.
fn ids_for_entries(v: Value) -> Value {
    match v {
        Value::Object(map)
            if map.contains_key("id")
                && map.contains_key("title")
                && map.contains_key("subtype") =>
        {
            map["id"].clone()
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, ids_for_entries(v)))
                .collect(),
        ),
        Value::Array(list) => Value::Array(list.into_iter().map(ids_for_entries).collect()),
        other => other,
    }
}

fn to_value<T: Serialize>(t: &T) -> Value {
    ids_for_entries(serde_json::to_value(t).expect("serializable"))
}

/// The finale's episode for the anchor: its last, where it has a count.
fn finale_episode(f: &Franchise) -> f64 {
    let entry = f
        .entries
        .iter()
        .chain(f.chain.iter().flat_map(|c| &c.entries))
        .find(|e| e.id == f.anchor);
    f64::from(entry.and_then(|e| e.episodes).unwrap_or(1))
}

/// Every decision for `input`, in both orders.
pub(crate) fn answer(input: &Input) -> Value {
    let f = &input.franchise;
    let id = f.anchor.as_str();
    let mut orders = serde_json::Map::new();
    for order in [WatchOrder::Linked, WatchOrder::Released] {
        let episode = finale_episode(f);
        let at = Finale {
            id,
            order,
            today: TODAY,
            episode,
            own: None,
        };
        orders.insert(
            to_value(&order).as_str().expect("a name").to_string(),
            json!({
                "page": to_value(&watch_order_page(f, &input.rows, id, order)),
                "franchise": to_value(&franchise_page(f, &input.rows, id, order)),
                "up_next": to_value(&player_up_next(f, &input.rows, at)),
                "episode": episode,
                "handoff": to_value(&handoff_from(f, id, order, TODAY, &input.played)),
            }),
        );
    }
    json!({ "link": to_value(&franchise_link_only(f, id)), "orders": orders })
}

/// Hand-off asks over a grid of rows.
fn ask_inputs() -> Vec<AskRow> {
    let mut out = Vec::new();
    for status in [Some("finished"), Some("current"), None] {
        for count in [None, Some(12)] {
            for ep_no in ["11", "12", "13", "12.5", "1-12", "", "x"] {
                for playable in [None, Some(11), Some(13)] {
                    for left_part_way in [false, true] {
                        out.push(AskRow {
                            row_id: format!("r{}", out.len()),
                            matched: Some(HandoffMatch {
                                id: "7".into(),
                                status: status.map(Into::into),
                                episode_count: count,
                            }),
                            ep_no: ep_no.into(),
                            playable_count: playable,
                            left_part_way,
                        });
                    }
                }
            }
        }
    }
    out.push(AskRow {
        row_id: "unmatched".into(),
        matched: None,
        ep_no: "12".into(),
        playable_count: None,
        left_part_way: false,
    });
    out
}

fn record() -> Value {
    let cases: Vec<Value> = inputs()
        .iter()
        .map(|i| json!({ "input": i.recorded(), "answer": answer(i) }))
        .collect();
    let asks: Vec<Value> = ask_inputs()
        .into_iter()
        .map(|row| json!({ "answer": handoff_ask(&row), "input": row }))
        .collect();
    json!({ "today": TODAY, "cases": cases, "asks": asks })
}

fn path() -> std::path::PathBuf {
    dir().join("parity.json")
}

#[test]
#[ignore = "writes the parity record"]
fn write_the_parity_record() {
    let body = serde_json::to_string(&record()).expect("json");
    std::fs::write(path(), body + "\n").expect("write");
}

#[test]
fn the_parity_record_is_what_these_modules_decide() {
    let recorded: Value =
        serde_json::from_slice(&std::fs::read(path()).expect("parity.json")).expect("parity json");
    // Read back through text as the record was: serde_json's reader may
    // land a float one step from the one written.
    let now: Value =
        serde_json::from_str(&serde_json::to_string(&record()).expect("json")).expect("json");
    let (Some(a), Some(b)) = (recorded["cases"].as_array(), now["cases"].as_array()) else {
        panic!("cases");
    };
    assert!(a.len() > 400, "the record holds the renderer's whole grid");
    assert_eq!(a.len(), b.len(), "the record holds every case");
    for (was, is) in a.iter().zip(b) {
        assert_eq!(was, is, "{}", is["input"]["name"]);
    }
    assert_eq!(recorded["asks"], now["asks"]);
}
