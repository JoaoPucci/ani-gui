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
//! This build declares no flag, and no kind a flag could be declared
//! with ([`decl::FlagKind`] has no variant outside the tests), so the
//! file it reads can change no flag, and the flags need no signature.
//! The file is signed all the same, for the notices it may carry:
//! [`signature`] keeps those only from a verified file, and reads the
//! flags whatever the verdict. A kind arrives with the change that
//! reads the flags themselves only under a verified signature (see
//! `docs/architecture.md`, "Remote flags"). Whether a flag's other
//! values are safe for the feature it gates is not something a type
//! can see: that is the declaration's `safe_because`, and review.
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

/// Every flag this build declares. Empty: no kind is declarable yet,
/// and a declaration comes with the code that reads it.
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
