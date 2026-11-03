use serde_json::json;

use super::{validate, FlagDecl, FlagKind, FlagValue};
use crate::flags::test_decls::TEST_DECLS;

const KILL: FlagDecl = TEST_DECLS[0];
const FALLBACK: FlagDecl = TEST_DECLS[1];

#[test]
fn a_kill_switch_ships_on_and_may_only_be_switched_off() {
    assert_eq!(KILL.default_value(), FlagValue::Bool(true));
    assert_eq!(
        KILL.allowed(),
        vec![FlagValue::Bool(true), FlagValue::Bool(false)]
    );
}

#[test]
fn a_fallback_ships_its_current_behaviour_and_may_only_select_an_earlier_one() {
    assert_eq!(FALLBACK.default_value(), FlagValue::Choice("new"));
    assert_eq!(
        FALLBACK.allowed(),
        vec![
            FlagValue::Choice("new"),
            FlagValue::Choice("old"),
            FlagValue::Choice("older"),
        ]
    );
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
fn a_kill_switch_accepts_a_boolean_and_nothing_else() {
    assert_eq!(KILL.accept(&json!(false)), Some(FlagValue::Bool(false)));
    assert_eq!(KILL.accept(&json!(true)), Some(FlagValue::Bool(true)));
    for wrong in [
        json!("false"),
        json!(0),
        json!(null),
        json!([false]),
        json!({ "value": false }),
    ] {
        assert_eq!(KILL.accept(&wrong), None, "{wrong}");
    }
}

#[test]
fn a_fallback_accepts_only_a_declared_name() {
    assert_eq!(
        FALLBACK.accept(&json!("old")),
        Some(FlagValue::Choice("old"))
    );
    assert_eq!(
        FALLBACK.accept(&json!("new")),
        Some(FlagValue::Choice("new"))
    );
    for wrong in [
        json!("newer"),
        json!("OLD"),
        json!(" old"),
        json!(""),
        json!(false),
        json!(["old"]),
    ] {
        assert_eq!(FALLBACK.accept(&wrong), None, "{wrong}");
    }
}

#[test]
fn an_accepted_choice_is_the_declarations_string_not_the_files() {
    let raw = json!("old");
    let Some(FlagValue::Choice(s)) = FALLBACK.accept(&raw) else {
        panic!("old is declared");
    };
    let declared = match FALLBACK.kind {
        FlagKind::Fallback { earlier, .. } => earlier[0],
        FlagKind::KillSwitch | FlagKind::Rollout => unreachable!(),
    };
    assert!(std::ptr::eq(s, declared));
}

#[test]
fn the_test_declarations_are_valid() {
    assert_eq!(validate(TEST_DECLS), Ok(()));
}

fn kill(name: &'static str) -> FlagDecl {
    FlagDecl {
        name,
        kind: FlagKind::KillSwitch,
        safe_because: "reason",
    }
}

fn fallback(current: &'static str, earlier: &'static [&'static str]) -> FlagDecl {
    FlagDecl {
        name: "f",
        kind: FlagKind::Fallback { current, earlier },
        safe_because: "reason",
    }
}

#[test]
fn validation_refuses_a_malformed_name() {
    for bad in ["", "Upper", "has space", "dash-ed", "dot.ted"] {
        assert!(validate(&[kill(bad)]).is_err(), "{bad:?}");
    }
    assert_eq!(validate(&[kill("ok_name_2")]), Ok(()));
}

#[test]
fn validation_refuses_a_name_declared_twice() {
    assert!(validate(&[kill("same"), kill("same")]).is_err());
}

#[test]
fn validation_refuses_a_declaration_without_its_reason() {
    let mut d = kill("k");
    d.safe_because = "  ";
    assert!(validate(&[d]).is_err());
}

#[test]
fn validation_refuses_a_fallback_with_nothing_earlier() {
    assert!(validate(&[fallback("new", &[])]).is_err());
}

#[test]
fn validation_refuses_a_fallback_that_lists_its_current_behaviour_as_earlier() {
    assert!(validate(&[fallback("new", &["old", "new"])]).is_err());
}

#[test]
fn validation_refuses_a_fallback_with_a_repeated_or_empty_value() {
    assert!(validate(&[fallback("new", &["old", "old"])]).is_err());
    assert!(validate(&[fallback("new", &[""])]).is_err());
    assert!(validate(&[fallback("", &["old"])]).is_err());
}

mod rollout {
    use super::super::{validate, FlagValue};
    use crate::flags::test_decls::{TEST_DECLS, TURN_ON, WITH_TURN_ON};

    #[test]
    fn a_rollout_ships_off_and_may_be_turned_on() {
        assert_eq!(TURN_ON.default_value(), FlagValue::Bool(false));
        assert_eq!(
            TURN_ON.allowed(),
            vec![FlagValue::Bool(false), FlagValue::Bool(true)]
        );
    }

    #[test]
    fn only_a_rollouts_on_value_needs_a_signature() {
        assert!(TURN_ON.signed_only(FlagValue::Bool(true)));
        assert!(!TURN_ON.signed_only(FlagValue::Bool(false)));
        for decl in TEST_DECLS {
            for v in decl.allowed() {
                assert!(!decl.signed_only(v), "{}: {v:?}", decl.name);
            }
        }
    }

    #[test]
    fn a_rollout_is_a_valid_declaration() {
        assert_eq!(validate(WITH_TURN_ON), Ok(()));
    }

    #[test]
    fn a_rollout_reads_as_off_until_turned_on() {
        use crate::flags::resolve::{parse_document, resolve};
        let none = resolve(WITH_TURN_ON, None);
        assert!(!none.enabled(&TURN_ON));
        let doc = parse_document(br#"{"flags":{"test_rollout":true}}"#).expect("doc");
        assert!(resolve(WITH_TURN_ON, Some(&doc)).enabled(&TURN_ON));
    }
}
