//! The production declaration list obeys the rules every declaration
//! must: these run over whatever `DECLARED` holds, so the first real
//! flag is checked the moment it is added.

use super::decl::validate;
use super::DECLARED;

#[test]
fn the_shipped_declarations_are_valid() {
    assert_eq!(validate(DECLARED), Ok(()));
}

#[test]
fn every_shipped_default_is_one_of_its_allowed_values() {
    for decl in DECLARED {
        assert!(
            decl.allowed().contains(&decl.default_value()),
            "{}: default {:?} is not among {:?}",
            decl.name,
            decl.default_value(),
            decl.allowed()
        );
    }
}

#[test]
fn no_flag_is_declared_under_the_notices_key() {
    use crate::flags::test_decls::TEST_DECLS;
    use crate::notices::NOTICES_KEY;
    assert!(DECLARED
        .iter()
        .chain(TEST_DECLS)
        .all(|d| d.name != NOTICES_KEY));
}
