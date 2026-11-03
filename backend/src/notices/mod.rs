//! The notification center's backend half: what the maintainer
//! publishes for it.
//!
//! Remote notices ride the flags file (see [`crate::flags`]): the same
//! launch request, the same cached copy, the same once-per-launch
//! decision. They sit under one reserved key of the file's `flags`
//! object, [`remote::NOTICES_KEY`], which flag resolution ignores
//! because no flag is declared under it. Nothing here makes a request
//! of its own.
//!
//! - [`remote`] — the typed, versioned block of notices and the
//!   donation-ask number, validated strictly: anything malformed is
//!   dropped with its reason, never passed on;
//! - [`serve`] — what the window is handed: the parsed block while the
//!   notification center's flag is on, nothing while it is off.

mod fields;
pub mod remote;
pub mod serve;
mod text_fields;

#[cfg(test)]
#[path = "serve_test.rs"]
pub(crate) mod serve_tests;

pub use remote::{parse, Parsed, Priority, RemoteNotice, RemoteNotices, NOTICES_KEY};
pub use serve::{served, NOTIFICATION_CENTER};
