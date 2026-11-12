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

/// Every declared flag is driven end to end, on the packaged app, in
/// each of its states: a spec of its own under `electron/e2e/flags/`,
/// named after the flag. This holds that the file is there; that it
/// launches the app with the flag off and with it on, and asserts what
/// each state shows, is the spec's own and review's.
#[test]
fn every_declared_flag_has_an_end_to_end_spec() {
    let specs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../electron/e2e/flags");
    for decl in DECLARED {
        let spec = specs.join(format!("{}.spec.ts", decl.name));
        assert!(
            spec.is_file(),
            "{} has no end-to-end spec at {}",
            decl.name,
            spec.display()
        );
    }
}

/// A backend built for test runs declares the probe the end-to-end
/// suite turns on and off, so the suite can prove on the packaged app
/// that a signed test-run file turns a flag on and every other leaves
/// it off. A build without the feature, every release, declares
/// no such flag.
#[test]
fn only_a_test_run_build_declares_the_probe() {
    let declared = DECLARED.iter().any(|d| d.name == "test_run_probe");
    assert_eq!(declared, cfg!(feature = "test-run-key"));
}
