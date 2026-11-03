//! Remote feature flags: a way to move a released build back to safer
//! behaviour without shipping another release.
//!
//! A broken update notifier is the case that motivated this: an
//! install that takes a notifier which can no longer announce
//! anything never hears of the release that fixes it. A flag the
//! maintainer can flip in a file on GitHub reaches every install on
//! its next launch, release or no release.
//!
//! The pieces:
//!
//! - [`decl`] — every flag the code knows, declared with its built-in
//!   default and the only values the remote file may select;
//! - [`resolve`] — the pure step from the file's bytes to one
//!   declared value per declared flag;
//! - [`cache`] — the last good copy kept on disk, and how long it
//!   stands in for an unreachable file;
//! - [`remote`] — the request for the file;
//! - [`refresh`] — the launch's decision within two seconds, and the
//!   background attempts that cache the file for the next launch;
//! - [`launch`] — the live file for a normal launch, the test's own
//!   file for a test run;
//! - [`store`] — what the rest of the backend reads: the flags decided
//!   once per launch.
//!
//! **The safety rule.** A flag may only move behaviour toward the
//! safer or older side: a kill switch turning a feature off, or a
//! return to a behaviour an earlier release had. The declaration's
//! type ([`decl::FlagKind`]) guarantees half of that: the default is
//! the build's own behaviour, and the file can only pick a value the
//! declaration lists — a kill switch's "off", or a fallback's named
//! earlier behaviours — because the value resolution returns is
//! borrowed from the declaration rather than copied from the file.
//! Whether the listed values really are the safer side is not
//! something a type can see: that is the declaration's
//! `safe_because`, and review. With every declaration held to that,
//! no value the file can select is riskier than the build's own
//! default, so the flags need no signature: anyone able to rewrite the
//! file in transit can do no more than switch a feature off. The file
//! is signed all the same, for the notices it may carry: [`signature`]
//! keeps those only from a verified file, and reads the flags whatever
//! the verdict. A flag kind with a riskier value — turning something
//! on, or relaxing a check — would need the flags themselves read only
//! under a verified signature, which no code does today (see
//! `docs/architecture.md`, "Remote flags").
//!
//! The backend owns the request; the renderer reads the resolved
//! values through `GET /api/flags`.

pub mod cache;
pub mod decl;
pub mod launch;
pub mod refresh;
pub mod remote;
pub mod resolve;
pub mod signature;
pub mod store;

pub use decl::{FlagDecl, FlagKind, FlagValue};
pub use resolve::Resolved;
pub use store::FlagStore;

/// Every flag this build declares. Empty until the first consumer
/// lands: a declaration comes with the code that reads it.
///
/// Declare each flag as a `pub const FlagDecl`, list it here, and read
/// it through that same constant with [`Resolved::enabled`] or
/// [`Resolved::choice`] — never through a name typed out again, which
/// a typo would turn into a silent default. Then register a test case
/// for its default and for every retreat in `matrix::CASES`: a test
/// fails while any value of a declared flag has none.
pub const DECLARED: &[FlagDecl] = &[];

#[cfg(test)]
pub(crate) mod test_decls;

#[cfg(test)]
pub(crate) mod matrix;

#[cfg(test)]
#[path = "declared_test.rs"]
mod declared_tests;
