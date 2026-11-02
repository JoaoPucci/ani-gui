//! From the remote file's bytes to one declared value per declared
//! flag. Pure: no I/O, no clock.
//!
//! The file is a JSON object with a `flags` object inside it:
//!
//! ```json
//! { "flags": { "some_feature": false } }
//! ```
//!
//! Everything that is not a usable value for a declared flag falls
//! back to that flag's built-in default, one flag at a time: an
//! unknown flag is ignored, and a wrong type or an undeclared value
//! leaves its flag at the default without affecting the others. A
//! document that is not that shape at all — malformed JSON, the wrong
//! top-level type, a body over [`MAX_BYTES`] — is no document, and the
//! caller decides what stands in for it.

use std::collections::BTreeMap;

use serde::Serialize;

use super::decl::{FlagDecl, FlagValue};

/// The largest flags file read. A few declared flags take well under
/// a kilobyte; a body this size is not a flags file.
pub const MAX_BYTES: usize = 64 * 1024;

/// The `flags` object of a parsed document.
pub type Document = serde_json::Map<String, serde_json::Value>;

/// One value per declared flag, every one of them declared.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Resolved(BTreeMap<&'static str, FlagValue>);

impl Resolved {
    /// The value of `name`, or `None` for a flag not declared.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<FlagValue> {
        self.0.get(name).copied()
    }

    /// Every resolved flag and its value, by name.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, FlagValue)> + '_ {
        self.0.iter().map(|(k, v)| (*k, *v))
    }
}

/// Parse a flags file. `None` unless `bytes` is at most [`MAX_BYTES`]
/// of JSON whose top level is an object holding a `flags` object.
#[must_use]
pub fn parse_document(bytes: &[u8]) -> Option<Document> {
    let _ = bytes;
    None
}

/// Resolve every declared flag against `doc`; with no document, every
/// flag takes its default.
#[must_use]
pub fn resolve(decls: &'static [FlagDecl], doc: Option<&Document>) -> Resolved {
    let _ = (decls, doc);
    Resolved::default()
}

#[cfg(test)]
#[path = "resolve_test.rs"]
mod tests;

#[cfg(test)]
#[path = "resolve_prop_test.rs"]
mod prop_tests;
