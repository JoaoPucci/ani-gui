use super::{parse_document, resolve, MAX_BYTES};
use crate::flags::decl::FlagValue;
use crate::flags::decl::{FlagDecl, FlagKind};
use crate::flags::test_decls::{A, B, TEST_DECLS};
use crate::flags::DECLARED;

const OFF: Option<FlagValue> = Some(FlagValue::Bool(false));
const ON: Option<FlagValue> = Some(FlagValue::Bool(true));

fn resolved(body: &str) -> (Option<FlagValue>, Option<FlagValue>) {
    let doc = parse_document(body.as_bytes());
    let r = resolve(TEST_DECLS, doc.as_ref());
    (r.get(A), r.get(B))
}

const DEFAULTS: (Option<FlagValue>, Option<FlagValue>) = (OFF, OFF);

#[test]
fn no_document_resolves_every_flag_to_its_default() {
    let r = resolve(TEST_DECLS, None);
    assert_eq!((r.get(A), r.get(B)), DEFAULTS);
    assert_eq!(r.iter().count(), TEST_DECLS.len());
}

/// The rule: a flag is off unless the file holds a `true` its
/// declaration allows, and a file that cannot be read leaves every
/// flag off. Over the test
/// declarations and over whatever this build declares.
#[test]
fn every_flag_reads_off_when_the_file_says_nothing_usable() {
    for decls in [TEST_DECLS, DECLARED] {
        for body in [None, Some(""), Some("not json"), Some(r#"{"flags":{}}"#)] {
            let doc = body.and_then(|b| parse_document(b.as_bytes()));
            let r = resolve(decls, doc.as_ref());
            for decl in decls {
                assert!(!r.enabled(decl), "{} with {body:?}", decl.name);
                assert_eq!(r.get(decl.name), OFF, "{} with {body:?}", decl.name);
            }
        }
    }
}

#[test]
fn declared_values_in_the_file_are_honoured() {
    assert_eq!(
        resolved(r#"{"flags":{"test_a":true,"test_b":true}}"#),
        (ON, ON)
    );
    assert_eq!(
        resolved(r#"{"flags":{"test_a":false,"test_b":true}}"#),
        (OFF, ON)
    );
}

#[test]
fn a_bad_value_falls_back_for_its_flag_alone() {
    assert_eq!(
        resolved(r#"{"flags":{"test_a":"yes","test_b":true}}"#),
        (OFF, ON)
    );
    assert_eq!(
        resolved(r#"{"flags":{"test_a":true,"test_b":1}}"#),
        (ON, OFF)
    );
}

#[test]
fn unknown_flags_are_ignored_and_never_resolved() {
    let doc = parse_document(br#"{"flags":{"not_declared":true,"test_a":true}}"#);
    let r = resolve(TEST_DECLS, doc.as_ref());
    assert_eq!(r.get("not_declared"), None);
    assert_eq!(r.get(A), ON);
    assert_eq!(r.iter().count(), TEST_DECLS.len());
}

#[test]
fn a_document_of_the_wrong_shape_is_no_document() {
    for body in [
        "",
        "not json",
        "{",
        "[]",
        "null",
        "true",
        r#""flags""#,
        r#"{"flags":[]}"#,
        r#"{"flags":true}"#,
        r#"{"flags":null}"#,
        r#"{"test_a":true}"#,
    ] {
        assert!(parse_document(body.as_bytes()).is_none(), "{body:?}");
        assert_eq!(resolved(body), DEFAULTS, "{body:?}");
    }
}

#[test]
fn other_top_level_keys_are_ignored() {
    assert_eq!(
        resolved(r#"{"comment":"x","flags":{"test_a":true}}"#),
        (ON, OFF)
    );
}

#[test]
fn a_body_over_the_cap_is_no_document_even_when_well_formed() {
    let pad = " ".repeat(MAX_BYTES);
    let body = format!(r#"{{"flags":{{"test_a":true}}}}{pad}"#);
    assert!(body.len() > MAX_BYTES);
    assert!(parse_document(body.as_bytes()).is_none());
    let fits = r#"{"flags":{"test_a":true}}"#;
    assert!(parse_document(fits.as_bytes()).is_some());
}

#[test]
fn deep_nesting_is_no_document_rather_than_a_crash() {
    let body = format!("{}{}", "[".repeat(10_000), "]".repeat(10_000));
    assert!(parse_document(body.as_bytes()).is_none());
}

#[test]
fn resolved_flags_serialise_as_a_name_to_value_object() {
    let doc = parse_document(br#"{"flags":{"test_b":true}}"#);
    let r = resolve(TEST_DECLS, doc.as_ref());
    assert_eq!(
        serde_json::to_value(&r).expect("serialise"),
        serde_json::json!({ "test_a": false, "test_b": true })
    );
}

const A_DECL: FlagDecl = TEST_DECLS[0];
const B_DECL: FlagDecl = TEST_DECLS[1];

#[test]
fn typed_reads_follow_the_file() {
    let doc = parse_document(br#"{"flags":{"test_a":true,"test_b":false}}"#);
    let r = resolve(TEST_DECLS, doc.as_ref());
    assert!(r.enabled(&A_DECL));
    assert!(!r.enabled(&B_DECL));
}

#[test]
fn typed_reads_are_off_without_a_document() {
    let r = resolve(TEST_DECLS, None);
    assert!(!r.enabled(&A_DECL));
    assert!(!r.enabled(&B_DECL));
}

#[test]
fn a_declaration_missing_from_the_set_reads_off() {
    // Resolved against an empty declaration list, as a consumer whose
    // declaration was left out of `DECLARED` would be.
    let r = resolve(
        &[],
        parse_document(br#"{"flags":{"test_a":true}}"#).as_ref(),
    );
    assert!(!r.enabled(&A_DECL));
}

#[test]
fn a_value_this_declaration_does_not_allow_reads_off() {
    // A set resolved against a same-named declaration with other
    // values must not leak them through the typed read.
    const WIDER: &[FlagDecl] = &[FlagDecl {
        name: A,
        kind: FlagKind::Probe {
            may_switch_on: true,
        },
        safe_because: "test-only",
    }];
    const NARROW: FlagDecl = FlagDecl {
        name: A,
        kind: FlagKind::Probe {
            may_switch_on: false,
        },
        safe_because: "test-only",
    };
    let r = resolve(
        WIDER,
        parse_document(br#"{"flags":{"test_a":true}}"#).as_ref(),
    );
    assert!(r.enabled(&WIDER[0]));
    assert!(!r.enabled(&NARROW));
}
