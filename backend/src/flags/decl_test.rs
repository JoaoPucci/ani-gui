use serde_json::json;

use super::{validate, FlagDecl, FlagKind, FlagValue};
use crate::flags::test_decls::TEST_DECLS;

const A: FlagDecl = TEST_DECLS[0];

fn probe(name: &'static str, may_switch_on: bool) -> FlagDecl {
    FlagDecl {
        name,
        kind: FlagKind::Probe { may_switch_on },
        safe_because: "reason",
    }
}

/// The rule: a flag ships off.
#[test]
fn every_declaration_ships_off() {
    for decl in TEST_DECLS
        .iter()
        .copied()
        .chain([probe("narrow", false), probe("wide", true)])
    {
        assert_eq!(
            decl.default_value(),
            FlagValue::Bool(false),
            "{}",
            decl.name
        );
    }
}

#[test]
fn a_flag_the_file_may_switch_on_allows_off_and_on() {
    assert_eq!(
        probe("p", true).allowed(),
        vec![FlagValue::Bool(false), FlagValue::Bool(true)]
    );
}

#[test]
fn a_flag_the_file_may_not_switch_on_allows_off_alone() {
    assert_eq!(probe("p", false).allowed(), vec![FlagValue::Bool(false)]);
    assert_eq!(probe("p", false).accept(&json!(true)), None);
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
        assert!(validate(&[probe(bad, true)]).is_err(), "{bad:?}");
    }
    assert_eq!(validate(&[probe("ok_name_2", true)]), Ok(()));
}

#[test]
fn validation_refuses_a_name_declared_twice() {
    assert!(validate(&[probe("same", true), probe("same", false)]).is_err());
}

#[test]
fn validation_refuses_a_declaration_without_its_reason() {
    let mut d = probe("k", true);
    d.safe_because = "  ";
    assert!(validate(&[d]).is_err());
}
