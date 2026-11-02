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
