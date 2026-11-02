use std::time::Duration;

use super::{FlagStore, Origin, SETTLE_CAP, SETTLE_LIMIT};
use crate::flags::decl::FlagValue;
use crate::flags::resolve::parse_document;
use crate::flags::test_decls::{FALLBACK, KILL, TEST_DECLS};

type Pair = (Option<FlagValue>, Option<FlagValue>);
const DEFAULTS: Pair = (Some(FlagValue::Bool(true)), Some(FlagValue::Choice("new")));
const FROM_DOC: Pair = (
    Some(FlagValue::Bool(false)),
    Some(FlagValue::Choice("older")),
);

fn pair(store: &FlagStore) -> Pair {
    let r = store.current();
    (r.get(KILL), r.get(FALLBACK))
}

fn doc() -> crate::flags::resolve::Document {
    parse_document(br#"{"flags":{"test_kill":false,"test_fallback":"older"}}"#).expect("doc")
}

#[test]
fn the_cap_is_two_seconds_and_the_limit_one_more() {
    assert_eq!(SETTLE_CAP, Duration::from_secs(2));
    assert_eq!(SETTLE_LIMIT, Duration::from_secs(3));
}

#[test]
fn a_new_store_is_pending_at_the_defaults() {
    let store = FlagStore::pending(TEST_DECLS);
    assert_eq!(pair(&store), DEFAULTS);
    assert_eq!(store.origin(), Origin::Pending);
    assert!(!store.is_settled());
    let fixed = FlagStore::fixed(TEST_DECLS);
    assert_eq!(pair(&fixed), DEFAULTS);
    assert_eq!(fixed.origin(), Origin::Defaults);
    assert!(fixed.is_settled());
}

#[test]
fn the_first_decision_stands_and_every_later_one_is_refused() {
    let store = FlagStore::pending(TEST_DECLS);
    assert!(store.settle(Some(&doc()), Origin::File));
    assert_eq!(pair(&store), FROM_DOC);
    assert!(!store.settle(None, Origin::Cache));
    assert_eq!(pair(&store), FROM_DOC);
    assert_eq!(store.origin(), Origin::File);
}

#[test]
fn a_fixed_store_refuses_a_decision() {
    let store = FlagStore::fixed(TEST_DECLS);
    assert!(!store.settle(Some(&doc()), Origin::File));
    assert_eq!(pair(&store), DEFAULTS);
}

#[test]
fn origins_have_the_names_the_route_uses() {
    let names: Vec<&str> = [
        Origin::Pending,
        Origin::File,
        Origin::Cache,
        Origin::Defaults,
        Origin::Test,
    ]
    .into_iter()
    .map(Origin::as_str)
    .collect();
    assert_eq!(names, ["pending", "file", "cache", "defaults", "test"]);
    assert_eq!(serde_json::to_value(Origin::Cache).expect("json"), "cache");
}

#[tokio::test(start_paused = true)]
async fn settled_answers_with_the_decision_once_it_is_made() {
    let store = FlagStore::pending(TEST_DECLS);
    let waiter = {
        let store = store.clone();
        tokio::spawn(async move { store.settled().await })
    };
    tokio::time::sleep(Duration::from_millis(500)).await;
    store.settle(Some(&doc()), Origin::File);
    let r = waiter.await.expect("join");
    assert_eq!((r.get(KILL), r.get(FALLBACK)), FROM_DOC);
}

#[tokio::test(start_paused = true)]
async fn settled_never_waits_past_its_limit_and_then_decides_the_defaults() {
    let store = FlagStore::pending(TEST_DECLS);
    let started = tokio::time::Instant::now();
    let r = store.settled().await;
    assert_eq!((r.get(KILL), r.get(FALLBACK)), DEFAULTS);
    assert!(started.elapsed() >= SETTLE_LIMIT);
    assert!(started.elapsed() < SETTLE_LIMIT + Duration::from_secs(1));
    // The defaults it answered with are the session's: a refresh that
    // decides late cannot change them under a reader.
    assert_eq!(store.origin(), Origin::Defaults);
    assert!(!store.settle(Some(&doc()), Origin::File));
    assert_eq!(pair(&store), DEFAULTS);
}
