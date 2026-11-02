//! Every declared flag is tested at its default and at every value the
//! file may select.
//!
//! A flag is a switch nobody flips until something is broken, so the
//! value it is flipped to is the one least likely to have run. Each
//! consumer therefore registers, in [`CASES`], one case per value of
//! its flag: a function that receives the launch's flags with that
//! value in force and asserts what the feature does under it. The
//! test below fails when a declared flag is missing a value, or a case
//! names a flag no longer declared, and runs every case.

use serde_json::Value;

use super::decl::{FlagDecl, FlagValue};
use super::resolve::{resolve, Document, Resolved};

/// One flag at one value, and the check of what the feature does then.
pub(crate) struct FlagCase {
    /// The flag's declaration — the same constant `DECLARED` lists.
    pub(crate) flag: FlagDecl,
    /// The value in force.
    pub(crate) value: FlagValue,
    /// Asserts the feature's behaviour with the flags `value` gives.
    pub(crate) check: fn(&Resolved),
}

/// Every consumer's cases. Empty until the first flag is declared.
pub(crate) const CASES: &[FlagCase] = &[];

/// What a set of cases leaves untested.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Gaps {
    /// Declared flags at values no case covers.
    pub(crate) untested: Vec<(&'static str, FlagValue)>,
    /// Cases for a flag `decls` does not declare, or for a value the
    /// declaration does not allow.
    pub(crate) stray: Vec<(&'static str, FlagValue)>,
}

/// Compare `cases` against every value of every flag in `decls`.
pub(crate) fn gaps(decls: &[FlagDecl], cases: &[FlagCase]) -> Gaps {
    let covered = |name: &str, value: FlagValue| {
        cases
            .iter()
            .any(|c| c.flag.name == name && c.value == value)
    };
    let declared = |name: &str, value: FlagValue| {
        decls
            .iter()
            .any(|d| d.name == name && d.allowed().contains(&value))
    };
    Gaps {
        untested: decls
            .iter()
            .flat_map(|d| d.allowed().into_iter().map(move |v| (d.name, v)))
            .filter(|(name, v)| !covered(name, *v))
            .collect(),
        stray: cases
            .iter()
            .map(|c| (c.flag.name, c.value))
            .filter(|(name, v)| !declared(name, *v))
            .collect(),
    }
}

/// Run every case against the flags its value gives.
pub(crate) fn run(decls: &[FlagDecl], cases: &[FlagCase]) {
    for case in cases {
        let doc = document(case.flag.name, case.value);
        (case.check)(&resolve(decls, Some(&doc)));
    }
}

/// A flags document holding `value` for `name`.
fn document(name: &str, value: FlagValue) -> Document {
    let raw = match value {
        FlagValue::Bool(b) => Value::Bool(b),
        FlagValue::Choice(c) => Value::String(c.to_string()),
    };
    std::iter::once((name.to_string(), raw)).collect()
}

#[cfg(test)]
#[path = "matrix_test.rs"]
mod tests;
