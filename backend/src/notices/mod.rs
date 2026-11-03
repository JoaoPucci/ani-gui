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
//!   dropped and logged, never passed on.

pub mod remote;

pub use remote::{parse, Parsed, Priority, RemoteNotice, RemoteNotices, NOTICES_KEY};
