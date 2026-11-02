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
//! - [`remote`] — the one request per launch;
//! - [`store`] — what the rest of the backend reads, and the launch
//!   refresh that fills it.
//!
//! **The safety rule.** A flag may only move behaviour toward the
//! safer or older side: a kill switch turning a feature off, or a
//! return to a behaviour an earlier release had. The declaration's
//! type carries that ([`decl::FlagKind`]): a kill switch accepts
//! only "off", a fallback only an earlier behaviour it names, and
//! resolution can return nothing else, because the value it returns is
//! borrowed from the declaration rather than copied from the file.
//! Because no value the file can select is riskier than the build's
//! own default, the file is not signed: anyone able to rewrite it in
//! transit can do no more than switch a feature off. A future flag that
//! needs a riskier value — turning something on, or relaxing a check —
//! must not be declared until the file is verified with the minisign
//! verifier proposed for the update path (see `docs/architecture.md`,
//! "Remote flags").
//!
//! The backend owns the request; the renderer reads the resolved
//! values through `GET /api/flags`.

pub mod decl;
pub mod resolve;

pub use decl::{FlagDecl, FlagKind, FlagValue};
pub use resolve::Resolved;

/// Every flag this build declares. Empty until the first consumer
/// lands: a declaration comes with the code that reads it.
pub const DECLARED: &[FlagDecl] = &[];

#[cfg(test)]
pub(crate) mod test_decls;

#[cfg(test)]
#[path = "declared_test.rs"]
mod declared_tests;
