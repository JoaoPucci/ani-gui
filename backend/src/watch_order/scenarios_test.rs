//! The acceptance scenarios' watch-order answers
//! (`tests/fixtures/watch-order/scenarios/*.json`): the frontend's CI
//! runs no Rust, so each scenario serves the answer recorded here for
//! the inputs it describes (`frontend/tests/acceptance/
//! watch-order-answers.ts`), and asserts only on what the renderer
//! draws from it. This module holds every recorded answer to what the
//! backend decides from those inputs, as the routes do
//! (`commands::watch_order`), and asserts the scenarios' decisions by
//! name.
//!
//! `write_the_scenario_answers` (ignored) fills in the answers after a
//! recording run of the scenarios; `WATCH_ORDER_SUITE=<file name>`
//! limits it to one suite.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::chain_end::Finale;
use super::handoff::{handoff_from, unclaimed_handoff};
use super::up_next::FinaleSelf;
use super::wire::{franchise_view, rail_view, show_view, up_next_view, RailRequest};
use super::{Franchise, HistoryRow, WatchOrder};
use crate::commands::watch_order::ids_of;

/// What the player read about the entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Own {
    pub(crate) status: Option<String>,
    pub(crate) episodes: Option<u32>,
}

/// One recorded read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Case {
    pub(crate) name: String,
    pub(crate) route: String,
    pub(crate) id: String,
    pub(crate) order: WatchOrder,
    /// The key of the franchise the walk answers, in [`Suite::franchises`].
    pub(crate) franchise: String,
    /// Each id a history row resumes, with its episode; none for a
    /// history that cannot be read.
    pub(crate) resumes: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) episode: Option<f64>,
    /// `up-next`: the player's own reading, where it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) own: Option<Own>,
    pub(crate) answer: Value,
}

/// One recorded rail request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Rail {
    pub(crate) name: String,
    pub(crate) request: RailRequest,
    pub(crate) answer: Value,
}

/// One acceptance test file's record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Suite {
    pub(crate) today: String,
    pub(crate) franchises: BTreeMap<String, Franchise>,
    pub(crate) cases: Vec<Case>,
    pub(crate) rails: Vec<Rail>,
}

fn dir() -> PathBuf {
    super::fixtures::dir().join("scenarios")
}

/// Every suite, by its file name.
pub(crate) fn suites() -> Vec<(String, Suite)> {
    let mut out: Vec<(String, Suite)> = std::fs::read_dir(dir())
        .expect("scenarios dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| {
            let name = p.file_stem().expect("stem").to_string_lossy().into_owned();
            let raw = std::fs::read(&p).expect("suite file");
            let suite: Suite =
                serde_json::from_slice(&raw).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            (name, suite)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The rows the resume lookup names for the franchise's titles, as
/// `commands::watch_order` reads them.
fn rows(f: &Franchise, resumes: Option<&BTreeMap<String, String>>) -> Vec<HistoryRow> {
    let Some(resumes) = resumes else {
        return Vec::new();
    };
    ids_of(f)
        .into_iter()
        .filter_map(|id| {
            resumes.get(&id).map(|ep| HistoryRow {
                ep_no: ep.clone(),
                kitsu_id: Some(id),
            })
        })
        .collect()
}

/// What the route answers for `case`, as JSON.
pub(crate) fn decide(suite: &Suite, case: &Case) -> Value {
    let f = suite
        .franchises
        .get(&case.franchise)
        .unwrap_or_else(|| panic!("{}: no franchise {}", case.name, case.franchise));
    let rows = rows(f, case.resumes.as_ref());
    let today = suite.today.as_str();
    let id = case.id.as_str();
    let json = |v: &dyn erased::Ser| v.value();
    match case.route.as_str() {
        "show" => json(&show_view(f, &rows, id, case.order, today)),
        "franchise" => json(&franchise_view(f, &rows, id, case.order, today)),
        "up-next" => {
            let own = case.own.clone().map(|o| FinaleSelf {
                status: o.status,
                episodes: o.episodes,
            });
            let at = Finale {
                id,
                order: case.order,
                today,
                episode: case.episode.expect("an up-next case's episode"),
                own: own.as_ref(),
            };
            json(&up_next_view(f, &rows, at))
        }
        "handoff" => {
            let candidate = handoff_from(f, id, case.order, today, &BTreeSet::new());
            let named = match (&candidate, &case.resumes) {
                (Some(h), Some(r)) => Some(r.contains_key(&h.next.id)),
                _ => None,
            };
            serde_json::json!({ "handoff": unclaimed_handoff(candidate, named) })
        }
        other => panic!("{}: no route {other}", case.name),
    }
}

/// `serde_json::to_value` behind a trait object, so one closure takes
/// every view.
mod erased {
    pub(super) trait Ser {
        fn value(&self) -> serde_json::Value;
    }
    impl<T: serde::Serialize> Ser for T {
        fn value(&self) -> serde_json::Value {
            serde_json::to_value(self).expect("a view serializes")
        }
    }
}

/// The answer recorded for scenario `scenario` of suite `suite` (an
/// acceptance test file): the one case it recorded, or, where it read
/// more than one, the one `route` and `id` name (` · show 42`, as the
/// recorded names end).
pub(crate) fn answer(suite: &str, scenario: &str, read: &str) -> Value {
    case(suite, &format!("{scenario} · {read}"))
}

/// The ids of a list of entries in an answer, in order.
pub(crate) fn ids(list: &Value) -> Vec<&str> {
    list.as_array()
        .unwrap_or_else(|| panic!("not a list: {list}"))
        .iter()
        .map(|e| e["id"].as_str().expect("an entry id"))
        .collect()
}

/// Every recorded case of suite `suite`, by name.
pub(crate) fn cases(suite: &str) -> Vec<(String, Value)> {
    suites()
        .into_iter()
        .find(|(n, _)| n == suite)
        .unwrap_or_else(|| panic!("no suite {suite}"))
        .1
        .cases
        .into_iter()
        .map(|c| (c.name, c.answer))
        .collect()
}

/// The recorded case `name` of suite `suite`.
pub(crate) fn case(suite: &str, name: &str) -> Value {
    let all = suites();
    let (_, s) = all
        .iter()
        .find(|(n, _)| n == suite)
        .unwrap_or_else(|| panic!("no suite {suite}"));
    s.cases
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| {
            let names: Vec<&str> = s.cases.iter().map(|c| c.name.as_str()).collect();
            panic!("no case {name:?} in {suite}; it has {names:#?}")
        })
        .answer
        .clone()
}

/// `v` read back through text, as a recorded answer is: serde_json's
/// reader may land a float one step from the one written.
fn through_text(v: &Value) -> Value {
    serde_json::from_str(&serde_json::to_string(v).expect("json")).expect("json")
}

#[test]
fn every_recorded_answer_is_what_the_backend_decides() {
    let all = suites();
    assert!(!all.is_empty(), "no scenario suites");
    for (name, suite) in &all {
        for case in &suite.cases {
            assert_eq!(
                case.answer,
                through_text(&decide(suite, case)),
                "{name}: {} (re-record: see frontend/tests/acceptance/watch-order-answers.ts)",
                case.name
            );
        }
        for rail in &suite.rails {
            assert_eq!(
                rail.answer,
                through_text(&serde_json::to_value(rail_view(&rail.request)).expect("rail")),
                "{name}: rail of {}",
                rail.name
            );
        }
    }
}

#[test]
#[ignore = "rewrites tests/fixtures/watch-order/scenarios/*.json"]
fn write_the_scenario_answers() {
    // `WATCH_ORDER_SUITE=<name>` rewrites that suite alone.
    let only = std::env::var("WATCH_ORDER_SUITE")
        .ok()
        .filter(|s| !s.is_empty());
    for (name, mut suite) in suites() {
        if only.as_ref().is_some_and(|o| *o != name) {
            continue;
        }
        let answers: Vec<Value> = suite.cases.iter().map(|c| decide(&suite, c)).collect();
        for (c, a) in suite.cases.iter_mut().zip(answers) {
            c.answer = a;
        }
        for r in &mut suite.rails {
            r.answer = serde_json::to_value(rail_view(&r.request)).expect("rail");
        }
        let mut out = serde_json::to_string_pretty(&suite).expect("suite");
        out.push('\n');
        std::fs::write(dir().join(format!("{name}.json")), out).expect("write suite");
    }
}
