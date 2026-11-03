//! Whatever the file holds, every declared flag resolves to one of
//! its declared values, and nothing else resolves at all.

use proptest::prelude::*;
use serde_json::{Map, Value};

use super::{parse_document, resolve, Resolved, MAX_BYTES};
use crate::flags::decl::{FlagDecl, FlagKind, FlagValue};
use crate::flags::test_decls::{FALLBACK, KILL, TEST_DECLS};

/// Strings that sit on and around the declared ones, so the generated
/// files hit allowed values often and near-misses more often.
fn near_string() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("new".to_string()),
        Just("old".to_string()),
        Just("older".to_string()),
        Just("newer".to_string()),
        Just("Old".to_string()),
        Just(String::new()),
        Just("true".to_string()),
        Just("false".to_string()),
        any::<String>(),
    ]
}

fn json_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        any::<f64>().prop_map(Value::from),
        near_string().prop_map(Value::String),
    ];
    leaf.prop_recursive(4, 32, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec((key(), inner), 0..6)
                .prop_map(|kv| Value::Object(kv.into_iter().collect())),
        ]
    })
}

fn key() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(KILL.to_string()),
        Just(FALLBACK.to_string()),
        Just("flags".to_string()),
        "[a-z_]{0,12}",
        any::<String>(),
    ]
}

/// A whole document: usually the right shape around generated flags,
/// sometimes any JSON at all.
fn document() -> impl Strategy<Value = Value> {
    prop_oneof![
        3 => prop::collection::vec((key(), json_value()), 0..8).prop_map(|kv| {
            let flags: Map<String, Value> = kv.into_iter().collect();
            serde_json::json!({ "flags": flags })
        }),
        1 => json_value(),
    ]
}

/// The values a declaration permits, written out from its kind
/// rather than through the code under test.
fn permitted(decl: &FlagDecl) -> Vec<FlagValue> {
    match decl.kind {
        FlagKind::KillSwitch => vec![FlagValue::Bool(true), FlagValue::Bool(false)],
        FlagKind::Rollout => vec![FlagValue::Bool(false), FlagValue::Bool(true)],
        FlagKind::Fallback { current, earlier } => std::iter::once(current)
            .chain(earlier.iter().copied())
            .map(FlagValue::Choice)
            .collect(),
    }
}

fn assert_only_declared(r: &Resolved) {
    let names: Vec<&str> = r.iter().map(|(k, _)| k).collect();
    let mut declared: Vec<&str> = TEST_DECLS.iter().map(|d| d.name).collect();
    declared.sort_unstable();
    assert_eq!(names, declared);
    for decl in TEST_DECLS {
        let v = r.get(decl.name).expect("every declared flag resolves");
        assert!(permitted(decl).contains(&v), "{}: {v:?}", decl.name);
    }
}

/// What a flag should resolve to, given the raw value the file held.
fn oracle(decl: &FlagDecl, raw: Option<&Value>) -> FlagValue {
    let default = permitted(decl)[0];
    let Some(raw) = raw else { return default };
    permitted(decl)
        .into_iter()
        .find(|p| match (p, raw) {
            (FlagValue::Bool(b), Value::Bool(r)) => b == r,
            (FlagValue::Choice(c), Value::String(r)) => c == r,
            _ => false,
        })
        .unwrap_or(default)
}

proptest! {
    #[test]
    fn any_bytes_resolve_to_declared_values_only(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        let doc = parse_document(&bytes);
        assert_only_declared(&resolve(TEST_DECLS, doc.as_ref()));
    }

    #[test]
    fn any_json_resolves_to_declared_values_only(doc in document()) {
        let bytes = serde_json::to_vec(&doc).expect("serialise");
        let parsed = parse_document(&bytes);
        assert_only_declared(&resolve(TEST_DECLS, parsed.as_ref()));
    }

    #[test]
    fn each_flag_takes_the_files_value_only_when_declared(doc in document()) {
        let bytes = serde_json::to_vec(&doc).expect("serialise");
        let r = resolve(TEST_DECLS, parse_document(&bytes).as_ref());
        let flags = doc.get("flags").and_then(Value::as_object).filter(|_| bytes.len() <= MAX_BYTES);
        for decl in TEST_DECLS {
            let raw = flags.and_then(|f| f.get(decl.name));
            prop_assert_eq!(r.get(decl.name), Some(oracle(decl, raw)), "{}", decl.name);
        }
    }

    #[test]
    fn a_huge_file_resolves_to_defaults(pad in (MAX_BYTES - 64)..(MAX_BYTES * 3)) {
        let body = format!(r#"{{"flags":{{"test_kill":false,"test_fallback":"old"}}}}{}"#, " ".repeat(pad));
        let r = resolve(TEST_DECLS, parse_document(body.as_bytes()).as_ref());
        assert_only_declared(&r);
        let expect_honoured = body.len() <= MAX_BYTES;
        prop_assert_eq!(r.get(KILL) == Some(FlagValue::Bool(false)), expect_honoured);
        prop_assert_eq!(r.get(FALLBACK) == Some(FlagValue::Choice("old")), expect_honoured);
    }
}
