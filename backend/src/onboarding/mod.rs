//! Feature tips after an update: the registry of tips each release
//! declares, the install's record of which it has seen, and the rules
//! that decide which are offered and pending.
//!
//! The pieces:
//!
//! - [`decl`] — the registry (`tips.json`) and the rules it holds to;
//! - [`release`] — the release numbers the tips' windows count in;
//! - [`eligibility`] — the pure rules: a tip's window, offered,
//!   pending, and what a launch adds to the record;
//! - [`state`] — the record on disk, and the launch's first-ever
//!   detection;
//! - [`view`] — what the routes send.
//!
//! The backend decides which tips are pending and records them seen;
//! the renderer decides when a pending tip's place is on screen,
//! draws it, and holds its words (`GET /api/onboarding`, in
//! `crate::api`). Only ids, kinds, pages and places cross: never words.

pub mod decl;
pub mod eligibility;
pub mod release;
pub mod state;
pub mod view;

use release::Release;

/// A test run's stand-in for the running release, so the end-to-end
/// suite can open a tip's window on a build whose own number is
/// outside it: honoured only in a test run (`ANI_GUI_UNATTENDED=1`)
/// of a backend built with the `test-run-key` feature, which no release
/// has.
pub const TEST_VERSION_ENV: &str = "ANI_GUI_TEST_ONBOARDING_VERSION";

/// The release the tips' windows are counted at for this process.
#[must_use]
pub fn running_version() -> Release {
    #[cfg(feature = "test-run-key")]
    {
        let unattended = std::env::var_os(crate::flags::launch::UNATTENDED_ENV);
        let stand_in = std::env::var(TEST_VERSION_ENV).ok();
        if let Some(r) = test_run_version(unattended.as_deref(), stand_in.as_deref()) {
            return r;
        }
    }
    built_version()
}

/// The build's own release number.
#[must_use]
pub fn built_version() -> Release {
    Release::parse(crate::VERSION).expect("CARGO_PKG_VERSION is a release number")
}

/// The stand-in a test run names, if this is a test run and it names a
/// release.
#[cfg(feature = "test-run-key")]
fn test_run_version(
    unattended: Option<&std::ffi::OsStr>,
    stand_in: Option<&str>,
) -> Option<Release> {
    if unattended != Some(std::ffi::OsStr::new("1")) {
        return None;
    }
    stand_in.and_then(Release::parse)
}

#[cfg(all(test, feature = "test-run-key"))]
#[path = "version_test.rs"]
mod version_tests;
