//! A verified file may turn a feature on; any other may only move a
//! flag toward its safer side, as before signing.

use proptest::prelude::*;
use serde_json::{json, Value};

use super::{admit_under, Verdict};
use crate::flags::decl::FlagValue;
use crate::flags::resolve::{resolve, Document};
use crate::flags::test_decls::{FALLBACK, KILL, ROLLOUT, TURN_ON, WITH_TURN_ON};

fn doc(v: Value) -> Document {
    v.as_object().expect("object").clone()
}

fn turned_on() -> Document {
    doc(json!({ ROLLOUT: true, KILL: false, FALLBACK: "old" }))
}

#[test]
fn a_verified_file_turns_a_rollout_on() {
    let admitted = admit_under(turned_on(), Verdict::Verified, WITH_TURN_ON);
    let flags = resolve(WITH_TURN_ON, Some(&admitted));
    assert!(flags.enabled(&TURN_ON));
    assert_eq!(flags.get(KILL), Some(FlagValue::Bool(false)));
    assert_eq!(flags.get(FALLBACK), Some(FlagValue::Choice("old")));
}

#[test]
fn an_unverified_file_cannot_turn_a_rollout_on_and_keeps_its_retreats() {
    for verdict in [Verdict::Unsigned, Verdict::Invalid] {
        let admitted = admit_under(turned_on(), verdict, WITH_TURN_ON);
        assert_eq!(admitted.get(ROLLOUT), None, "{verdict:?}");
        let flags = resolve(WITH_TURN_ON, Some(&admitted));
        assert!(!flags.enabled(&TURN_ON), "{verdict:?}");
        assert_eq!(flags.get(KILL), Some(FlagValue::Bool(false)), "{verdict:?}");
        assert_eq!(
            flags.get(FALLBACK),
            Some(FlagValue::Choice("old")),
            "{verdict:?}"
        );
    }
}

#[test]
fn an_unverified_file_may_still_say_a_rollout_is_off() {
    let off = doc(json!({ ROLLOUT: false }));
    let admitted = admit_under(off.clone(), Verdict::Unsigned, WITH_TURN_ON);
    assert_eq!(admitted, off);
}

#[test]
fn a_value_no_declaration_reads_is_left_for_resolution_to_ignore() {
    let odd = doc(json!({ ROLLOUT: "yes", "unknown": true }));
    let admitted = admit_under(odd.clone(), Verdict::Unsigned, WITH_TURN_ON);
    assert_eq!(admitted, odd);
    assert!(!resolve(WITH_TURN_ON, Some(&admitted)).enabled(&TURN_ON));
}

fn any_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| json!(n)),
        prop::sample::select(vec!["new", "old", "older", "true", "on", ""]).prop_map(|s| json!(s)),
    ]
}

fn any_doc() -> impl Strategy<Value = Document> {
    let names = prop::sample::select(vec![KILL, FALLBACK, ROLLOUT, "notices", "other"]);
    prop::collection::btree_map(names, any_value(), 0..5)
        .prop_map(|m| m.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

proptest! {
    /// Whatever an unverified file says, no flag resolves to a value
    /// only a signature may select, and nothing else it selects is lost.
    #[test]
    fn an_unverified_file_never_resolves_a_signed_only_value(
        d in any_doc(),
        verdict in prop::sample::select(vec![Verdict::Unsigned, Verdict::Invalid]),
    ) {
        let admitted = admit_under(d.clone(), verdict, WITH_TURN_ON);
        let flags = resolve(WITH_TURN_ON, Some(&admitted));
        for decl in WITH_TURN_ON {
            let v = flags.get(decl.name).expect("declared");
            prop_assert!(!decl.signed_only(v), "{}: {:?}", decl.name, v);
        }
        let whole = resolve(WITH_TURN_ON, Some(&d));
        for decl in WITH_TURN_ON {
            let v = whole.get(decl.name).expect("declared");
            if !decl.signed_only(v) {
                prop_assert_eq!(flags.get(decl.name), Some(v));
            }
        }
    }

    /// A verified file resolves exactly as it reads.
    #[test]
    fn a_verified_file_resolves_as_it_reads(d in any_doc()) {
        let admitted = admit_under(d.clone(), Verdict::Verified, WITH_TURN_ON);
        prop_assert_eq!(resolve(WITH_TURN_ON, Some(&admitted)), resolve(WITH_TURN_ON, Some(&d)));
    }
}
