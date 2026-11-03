use super::{parse_document, resolve, MAX_BYTES};
use crate::flags::decl::FlagValue;
use crate::flags::decl::{FlagDecl, FlagKind};
use crate::flags::test_decls::{FALLBACK, KILL, TEST_DECLS};

fn resolved(body: &str) -> (Option<FlagValue>, Option<FlagValue>) {
    let doc = parse_document(body.as_bytes());
    let r = resolve(TEST_DECLS, doc.as_ref());
    (r.get(KILL), r.get(FALLBACK))
}

const DEFAULTS: (Option<FlagValue>, Option<FlagValue>) =
    (Some(FlagValue::Bool(true)), Some(FlagValue::Choice("new")));

#[test]
fn no_document_resolves_every_flag_to_its_default() {
    let r = resolve(TEST_DECLS, None);
    assert_eq!((r.get(KILL), r.get(FALLBACK)), DEFAULTS);
    assert_eq!(r.iter().count(), TEST_DECLS.len());
}

#[test]
fn declared_values_in_the_file_are_honoured() {
    assert_eq!(
        resolved(r#"{"flags":{"test_kill":false,"test_fallback":"older"}}"#),
        (
            Some(FlagValue::Bool(false)),
            Some(FlagValue::Choice("older"))
        )
    );
}

#[test]
fn a_bad_value_falls_back_for_its_flag_alone() {
    assert_eq!(
        resolved(r#"{"flags":{"test_kill":"no","test_fallback":"old"}}"#),
        (Some(FlagValue::Bool(true)), Some(FlagValue::Choice("old")))
    );
    assert_eq!(
        resolved(r#"{"flags":{"test_kill":false,"test_fallback":"newest"}}"#),
        (Some(FlagValue::Bool(false)), Some(FlagValue::Choice("new")))
    );
}

#[test]
fn unknown_flags_are_ignored_and_never_resolved() {
    let doc = parse_document(br#"{"flags":{"not_declared":false,"test_kill":false}}"#);
    let r = resolve(TEST_DECLS, doc.as_ref());
    assert_eq!(r.get("not_declared"), None);
    assert_eq!(r.get(KILL), Some(FlagValue::Bool(false)));
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
        "false",
        r#""flags""#,
        r#"{"flags":[]}"#,
        r#"{"flags":false}"#,
        r#"{"flags":null}"#,
        r#"{"test_kill":false}"#,
    ] {
        assert!(parse_document(body.as_bytes()).is_none(), "{body:?}");
        assert_eq!(resolved(body), DEFAULTS, "{body:?}");
    }
}

#[test]
fn other_top_level_keys_are_ignored() {
    assert_eq!(
        resolved(r#"{"comment":"x","flags":{"test_kill":false}}"#),
        (Some(FlagValue::Bool(false)), Some(FlagValue::Choice("new")))
    );
}

#[test]
fn a_body_over_the_cap_is_no_document_even_when_well_formed() {
    let pad = " ".repeat(MAX_BYTES);
    let body = format!(r#"{{"flags":{{"test_kill":false}}}}{pad}"#);
    assert!(body.len() > MAX_BYTES);
    assert!(parse_document(body.as_bytes()).is_none());
    let fits = r#"{"flags":{"test_kill":false}}"#;
    assert!(parse_document(fits.as_bytes()).is_some());
}

#[test]
fn deep_nesting_is_no_document_rather_than_a_crash() {
    let body = format!("{}{}", "[".repeat(10_000), "]".repeat(10_000));
    assert!(parse_document(body.as_bytes()).is_none());
}

#[test]
fn resolved_flags_serialise_as_a_name_to_value_object() {
    let doc = parse_document(br#"{"flags":{"test_fallback":"old"}}"#);
    let r = resolve(TEST_DECLS, doc.as_ref());
    assert_eq!(
        serde_json::to_value(&r).expect("serialise"),
        serde_json::json!({ "test_kill": true, "test_fallback": "old" })
    );
}

const KILL_DECL: FlagDecl = TEST_DECLS[0];
const FALLBACK_DECL: FlagDecl = TEST_DECLS[1];

#[test]
fn typed_reads_follow_the_file() {
    let doc = parse_document(br#"{"flags":{"test_kill":false,"test_fallback":"old"}}"#);
    let r = resolve(TEST_DECLS, doc.as_ref());
    assert!(!r.enabled(&KILL_DECL));
    assert_eq!(r.choice(&FALLBACK_DECL), Some("old"));
}

#[test]
fn typed_reads_default_without_a_document() {
    let r = resolve(TEST_DECLS, None);
    assert!(r.enabled(&KILL_DECL));
    assert_eq!(r.choice(&FALLBACK_DECL), Some("new"));
}

#[test]
fn a_declaration_missing_from_the_set_reads_as_its_default() {
    // Resolved against an empty declaration list, as a consumer whose
    // declaration was left out of `DECLARED` would be.
    let r = resolve(
        &[],
        parse_document(br#"{"flags":{"test_kill":false}}"#).as_ref(),
    );
    assert!(r.enabled(&KILL_DECL));
    assert_eq!(r.choice(&FALLBACK_DECL), Some("new"));
    let other = FlagDecl {
        name: "other",
        kind: FlagKind::Fallback {
            current: "b",
            earlier: &["a"],
        },
        safe_because: "test-only",
    };
    assert_eq!(r.choice(&other), Some("b"));
}

#[test]
fn reading_a_declaration_as_the_other_kind_switches_nothing() {
    let doc = parse_document(br#"{"flags":{"test_kill":false,"test_fallback":"old"}}"#);
    let r = resolve(TEST_DECLS, doc.as_ref());
    assert!(r.enabled(&FALLBACK_DECL));
    assert_eq!(r.choice(&KILL_DECL), None);
}

#[test]
fn a_value_this_declaration_does_not_allow_reads_as_its_default() {
    // A set resolved against a same-named declaration with other
    // values must not leak them through the typed read.
    const WIDER: &[FlagDecl] = &[FlagDecl {
        name: FALLBACK,
        kind: FlagKind::Fallback {
            current: "new",
            earlier: &["elsewhere"],
        },
        safe_because: "test-only",
    }];
    let r = resolve(
        WIDER,
        parse_document(br#"{"flags":{"test_fallback":"elsewhere"}}"#).as_ref(),
    );
    assert_eq!(r.choice(&FALLBACK_DECL), Some("new"));
}
