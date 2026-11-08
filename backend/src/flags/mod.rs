//! Remote feature flags: values a released build reads from a file on
//! GitHub at launch, so a feature's flag can change without another
//! release. A flag the maintainer changes in that file reaches every
//! install on its next launch.
//!
//! The pieces:
//!
//! - [`decl`] — every flag the code knows, declared with its built-in
//!   default and the only values the remote file may select;
//! - [`resolve`] — the pure step from the file's bytes to one
//!   declared value per declared flag;
//! - [`signature`] — the file's detached signature, and what an
//!   unverified file may not say;
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
//! **The rule.** Flags are default OFF, and any read failure means
//! OFF. A flag's default is `false` ([`decl::FlagDecl::default_value`]),
//! and no file, an unreadable or malformed one, a value of the wrong
//! type or one the declaration does not allow, and a flag the file
//! does not mention all leave it there. The value resolution returns
//! is borrowed from the declaration, never copied from the file.
//!
//! **What a signature adds.** The one kind a flag is declared with,
//! [`decl::FlagKind::Rollout`], gates a feature that ships off, and
//! its `true` turns that feature on. Only a file whose minisign
//! signature verifies under a key compiled into the build may do that:
//! [`signature::admit`] drops every `true` from any other file before
//! the flags are decided or cached, and keeps the rest. So an unsigned
//! file, or anyone able to rewrite it in transit, can only leave a
//! feature off, or bring back a file and signature the maintainer
//! signed before, which verifies again. Whether turning a flag's
//! feature on is safe is not something a type can see: that is the
//! declaration's `safe_because`, and review.
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

/// Every flag this build declares. Empty: a declaration comes with the
/// code that reads it.
///
/// Declare each flag as a `pub const FlagDecl`, list it here, and read
/// it through that same constant with [`Resolved::enabled`] — never
/// through a name typed out again, which a typo would turn into a
/// silent off. Then register a test case for its default and for every
/// other value it allows in `matrix::CASES`: a test fails while any
/// value of a declared flag has none.
pub const DECLARED: &[FlagDecl] = &[];

#[cfg(test)]
pub(crate) mod test_decls;

#[cfg(test)]
pub(crate) mod matrix;

#[cfg(test)]
#[path = "declared_test.rs"]
mod declared_tests;
