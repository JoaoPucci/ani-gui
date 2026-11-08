//! Whatever the file holds, every declared flag resolves to one of
//! its declared values, and nothing else resolves at all.

use proptest::prelude::*;
use serde_json::{Map, Value};

use super::{parse_document, resolve, Resolved, MAX_BYTES};
use crate::flags::decl::{FlagDecl, FlagKind, FlagValue};
use crate::flags::test_decls::{A, B, TEST_DECLS};

/// Strings that spell a boolean without being one, so the generated
/// files hit near-misses often.
fn near_string() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("true".to_string()),
        Just("false".to_string()),
        Just("on".to_string()),
        Just("True".to_string()),
        Just(String::new()),
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
        Just(A.to_string()),
        Just(B.to_string()),
        Just(NARROW.to_string()),
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

/// The declarations these properties resolve against: the test
/// declarations, which the file may switch on, and one it may not, the
/// shape a flag takes while the file is unsigned.
const PROP_DECLS: &[FlagDecl] = &[
    TEST_DECLS[0],
    TEST_DECLS[1],
    FlagDecl {
        name: NARROW,
        kind: FlagKind::Probe {
            may_switch_on: false,
        },
        safe_because: "test-only: the flag gates nothing",
    },
];
const NARROW: &str = "test_narrow";

/// The values a declaration permits, written out from its kind rather
/// than read through the code under test: off first, the default.
fn permitted(decl: &FlagDecl) -> Vec<FlagValue> {
    let FlagKind::Probe { may_switch_on } = decl.kind;
    let mut v = vec![FlagValue::Bool(false)];
    if may_switch_on {
        v.push(FlagValue::Bool(true));
    }
    v
}

fn assert_only_declared(r: &Resolved) {
    let names: Vec<&str> = r.iter().map(|(k, _)| k).collect();
    let mut declared: Vec<&str> = PROP_DECLS.iter().map(|d| d.name).collect();
    declared.sort_unstable();
    assert_eq!(names, declared);
    for decl in PROP_DECLS {
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
        .find(|p| raw.as_bool().map(FlagValue::Bool) == Some(*p))
        .unwrap_or(default)
}

proptest! {
    #[test]
    fn any_bytes_resolve_to_declared_values_only(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        let doc = parse_document(&bytes);
        assert_only_declared(&resolve(PROP_DECLS, doc.as_ref()));
    }

    #[test]
    fn any_json_resolves_to_declared_values_only(doc in document()) {
        let bytes = serde_json::to_vec(&doc).expect("serialise");
        let parsed = parse_document(&bytes);
        assert_only_declared(&resolve(PROP_DECLS, parsed.as_ref()));
    }

    #[test]
    fn each_flag_takes_the_files_value_only_when_declared(doc in document()) {
        let bytes = serde_json::to_vec(&doc).expect("serialise");
        let r = resolve(PROP_DECLS, parse_document(&bytes).as_ref());
        let flags = doc.get("flags").and_then(Value::as_object).filter(|_| bytes.len() <= MAX_BYTES);
        for decl in PROP_DECLS {
            let raw = flags.and_then(|f| f.get(decl.name));
            prop_assert_eq!(r.get(decl.name), Some(oracle(decl, raw)), "{}", decl.name);
        }
    }

    /// The rule: a flag reads on only when the file held exactly
    /// `true` for it and its declaration allows that; anything else, a
    /// failed read included, is off.
    #[test]
    fn a_flag_reads_on_only_for_an_exact_true(doc in document()) {
        let bytes = serde_json::to_vec(&doc).expect("serialise");
        let r = resolve(PROP_DECLS, parse_document(&bytes).as_ref());
        let flags = doc.get("flags").and_then(Value::as_object).filter(|_| bytes.len() <= MAX_BYTES);
        for decl in PROP_DECLS {
            let raw = flags.and_then(|f| f.get(decl.name));
            let may_be_on = permitted(decl).contains(&FlagValue::Bool(true));
            prop_assert_eq!(r.enabled(decl), may_be_on && raw == Some(&Value::Bool(true)), "{}", decl.name);
        }
    }

    #[test]
    fn a_huge_file_resolves_to_defaults(pad in (MAX_BYTES - 64)..(MAX_BYTES * 3)) {
        let body = format!(r#"{{"flags":{{"test_a":true,"test_b":true}}}}{}"#, " ".repeat(pad));
        let r = resolve(PROP_DECLS, parse_document(body.as_bytes()).as_ref());
        assert_only_declared(&r);
        let expect_honoured = body.len() <= MAX_BYTES;
        prop_assert_eq!(r.get(A) == Some(FlagValue::Bool(true)), expect_honoured);
        prop_assert_eq!(r.get(B) == Some(FlagValue::Bool(true)), expect_honoured);
    }
}
