use serde_json::json;

use super::{validate, FlagDecl, FlagKind, FlagValue};
use crate::flags::test_decls::TEST_DECLS;

const A: FlagDecl = TEST_DECLS[0];

fn rollout(name: &'static str) -> FlagDecl {
    FlagDecl {
        name,
        kind: FlagKind::Rollout,
        safe_because: "reason",
    }
}

/// The rule: a flag ships off.
#[test]
fn every_declaration_ships_off() {
    for decl in TEST_DECLS.iter().copied().chain([rollout("other")]) {
        assert_eq!(
            decl.default_value(),
            FlagValue::Bool(false),
            "{}",
            decl.name
        );
    }
}

#[test]
fn a_rollout_allows_off_and_on() {
    assert_eq!(
        rollout("p").allowed(),
        vec![FlagValue::Bool(false), FlagValue::Bool(true)]
    );
}

/// Turning a rollout on is riskier than the build's own default, so
/// only a verified file may do it; off is the default, and any file
/// may say so.
#[test]
fn only_a_rollouts_on_needs_a_signature() {
    let decl = rollout("p");
    assert!(decl.signed_only(FlagValue::Bool(true)));
    assert!(!decl.signed_only(FlagValue::Bool(false)));
}

#[test]
fn every_test_default_is_one_of_its_allowed_values() {
    for decl in TEST_DECLS {
        assert!(
            decl.allowed().contains(&decl.default_value()),
            "{}",
            decl.name
        );
    }
}

#[test]
fn a_flag_accepts_a_boolean_and_nothing_else() {
    assert_eq!(A.accept(&json!(false)), Some(FlagValue::Bool(false)));
    assert_eq!(A.accept(&json!(true)), Some(FlagValue::Bool(true)));
    for wrong in [
        json!("true"),
        json!("on"),
        json!(1),
        json!(null),
        json!([true]),
        json!({ "value": true }),
    ] {
        assert_eq!(A.accept(&wrong), None, "{wrong}");
    }
}

#[test]
fn the_test_declarations_are_valid() {
    assert_eq!(validate(TEST_DECLS), Ok(()));
}

#[test]
fn validation_refuses_a_malformed_name() {
    for bad in ["", "Upper", "has space", "dash-ed", "dot.ted"] {
        assert!(validate(&[rollout(bad)]).is_err(), "{bad:?}");
    }
    assert_eq!(validate(&[rollout("ok_name_2")]), Ok(()));
}

#[test]
fn validation_refuses_a_name_declared_twice() {
    assert!(validate(&[rollout("same"), rollout("same")]).is_err());
}

#[test]
fn validation_refuses_a_declaration_without_its_reason() {
    let mut d = rollout("k");
    d.safe_because = "  ";
    assert!(validate(&[d]).is_err());
}
