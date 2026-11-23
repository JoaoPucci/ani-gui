//! Only a verified file may turn a feature on. Any file may leave one
//! off, and whatever else an unverified file says, nothing it leaves
//! after admission can turn a flag on.

use proptest::prelude::*;
use serde_json::{json, Value};

use super::{admit, holds_signed_content, Verdict};
use crate::flags::decl::{FlagDecl, FlagKind, FlagValue};
use crate::flags::resolve::{resolve, Document};
use crate::flags::test_decls::{A, B, TEST_DECLS};

const A_DECL: FlagDecl = TEST_DECLS[0];

fn doc(v: Value) -> Document {
    v.as_object().expect("object").clone()
}

fn turned_on() -> Document {
    doc(json!({ A: true, B: false }))
}

#[test]
fn a_verified_file_turns_a_rollout_on() {
    let admitted = admit(turned_on(), Verdict::Verified);
    assert_eq!(admitted, turned_on());
    let flags = resolve(TEST_DECLS, Some(&admitted));
    assert!(flags.enabled(&A_DECL));
    assert_eq!(flags.get(B), Some(FlagValue::Bool(false)));
}

#[test]
fn an_unverified_file_cannot_turn_a_rollout_on() {
    for verdict in [Verdict::Unsigned, Verdict::Invalid] {
        let admitted = admit(turned_on(), verdict);
        assert_eq!(admitted.get(A), None, "{verdict:?}");
        assert_eq!(admitted.get(B), Some(&json!(false)), "{verdict:?}");
        let flags = resolve(TEST_DECLS, Some(&admitted));
        assert!(!flags.enabled(&A_DECL), "{verdict:?}");
    }
}

#[test]
fn an_unverified_file_may_still_say_a_rollout_is_off() {
    let off = doc(json!({ A: false, B: false }));
    assert_eq!(admit(off.clone(), Verdict::Unsigned), off);
}

/// Admission does not read the declarations: it drops every `true`.
/// That is the same rule as dropping every value only a signature may
/// select, for as long as `true` is the only such value of every kind.
/// A kind added to [`FlagKind`] fails to compile here until it is
/// listed, and the test then checks the rule still holds for it.
#[test]
fn true_is_the_only_value_any_kind_needs_a_signature_for() {
    let every_kind = |k: FlagKind| match k {
        FlagKind::Rollout => [FlagKind::Rollout],
    };
    for kind in every_kind(FlagKind::Rollout) {
        let decl = FlagDecl {
            name: "k",
            kind,
            safe_because: "test-only",
        };
        for v in decl.allowed() {
            assert_eq!(
                decl.signed_only(v),
                v == FlagValue::Bool(true),
                "{kind:?}: {v:?}"
            );
        }
    }
}

#[test]
fn a_value_no_declaration_reads_is_left_for_resolution_to_ignore() {
    let odd = doc(json!({ A: "yes", "unknown": 1 }));
    let admitted = admit(odd.clone(), Verdict::Unsigned);
    assert_eq!(admitted, odd);
    assert!(!resolve(TEST_DECLS, Some(&admitted)).enabled(&A_DECL));
}

fn any_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| json!(n)),
        prop::sample::select(vec!["true", "on", "false", ""]).prop_map(|s| json!(s)),
        any::<bool>().prop_map(|b| json!([b])),
    ]
}

fn any_doc() -> impl Strategy<Value = Document> {
    let names = prop::sample::select(vec![A, B, "other", "flags"]);
    prop::collection::btree_map(names, any_value(), 0..5)
        .prop_map(|m| m.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

proptest! {
    /// Whatever an unverified file says, no flag resolves to a value
    /// only a signature may select, and every other value it selects
    /// is kept.
    #[test]
    fn an_unverified_file_never_resolves_a_signed_only_value(
        d in any_doc(),
        verdict in prop::sample::select(vec![Verdict::Unsigned, Verdict::Invalid]),
    ) {
        let admitted = admit(d.clone(), verdict);
        let flags = resolve(TEST_DECLS, Some(&admitted));
        let whole = resolve(TEST_DECLS, Some(&d));
        for decl in TEST_DECLS {
            let v = flags.get(decl.name).expect("declared");
            prop_assert!(!decl.signed_only(v), "{}: {:?}", decl.name, v);
            prop_assert!(!flags.enabled(decl), "{}", decl.name);
            let read = whole.get(decl.name).expect("declared");
            if !decl.signed_only(read) {
                prop_assert_eq!(v, read, "{}", decl.name);
            }
        }
    }

    /// A verified file is admitted whole, so it resolves exactly as
    /// it reads.
    #[test]
    fn a_verified_file_resolves_as_it_reads(d in any_doc()) {
        let admitted = admit(d.clone(), Verdict::Verified);
        prop_assert_eq!(&admitted, &d);
        prop_assert_eq!(resolve(TEST_DECLS, Some(&admitted)), resolve(TEST_DECLS, Some(&d)));
    }

    /// What an unverified file leaves after admission never looks like
    /// a verified one, so the cache never marks it as one.
    #[test]
    fn an_unverified_file_never_holds_signed_content(
        d in any_doc(),
        verdict in prop::sample::select(vec![Verdict::Unsigned, Verdict::Invalid]),
    ) {
        prop_assert!(!holds_signed_content(&admit(d, verdict)));
    }

    /// Admitting an admitted document again changes nothing, which is
    /// what lets the cache admit an unmarked entry again on reading.
    #[test]
    fn admission_is_idempotent(
        d in any_doc(),
        verdict in prop::sample::select(vec![Verdict::Verified, Verdict::Unsigned, Verdict::Invalid]),
    ) {
        let once = admit(d, verdict);
        prop_assert_eq!(admit(once.clone(), verdict), once.clone());
        if verdict != Verdict::Verified {
            prop_assert_eq!(admit(once.clone(), Verdict::Unsigned), once);
        }
    }
}
