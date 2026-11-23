//! The flag only a test-run build declares: a backend built with the
//! `test-run-key` feature, as the backend's tests and the end-to-end
//! suite's package are (`pnpm run package:e2e`), and no release is.
//!
//! It gates nothing. It exists so the end-to-end suite can prove, on
//! the packaged app, that a test run's file signed by the test-run key
//! turns a flag on and that every other file, and no file, leaves it
//! off (`electron/e2e/flags/test_run_probe.spec.ts`), whether or not
//! any real flag is declared.

use super::decl::{FlagDecl, FlagKind};

/// The probe.
pub const PROBE: FlagDecl = FlagDecl {
    name: "test_run_probe",
    kind: FlagKind::Rollout,
    safe_because: "it gates nothing, and only a test-run build, which no release is, declares it",
};

/// The probe's flag-matrix check with it off: nothing reads it, so
/// the flags carrying the value in force is all there is to see.
#[cfg(test)]
pub(crate) fn check_off(r: &super::Resolved) {
    assert!(!r.enabled(&PROBE));
}

/// The same, with it on.
#[cfg(test)]
pub(crate) fn check_on(r: &super::Resolved) {
    assert!(r.enabled(&PROBE));
}
