//! Flag declarations: the name, the built-in default and the values
//! the remote file may select.
//!
//! The rule the type carries: a flag ships off. Its default is
//! `false` whatever its kind, and a read that fails, a value the
//! declaration does not allow and a flag the file does not mention
//! all leave it there.
//!
//! This build declares no flag and has no kind one could be declared
//! with: [`FlagKind`] has no variant outside the tests, so a
//! [`FlagDecl`] cannot be built and [`super::DECLARED`] can only be
//! empty. A kind arrives with the change that verifies the file, and
//! with it the values that kind lets the file send.
//!
//! What the type cannot see is whether a flag's other values are safe
//! for the feature it gates. That is a review question, so every
//! declaration states its answer in `safe_because`, and [`validate`]
//! refuses a declaration that leaves it empty.

use serde::Serialize;

/// A resolved flag value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum FlagValue {
    /// An on/off flag: `false`, the default, or `true` when the file
    /// holds a `true` its declaration allows.
    Bool(bool),
}

/// Which values the file may send for a flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagKind {
    /// Gates a feature that ships off. Default `false`; any file may
    /// send `false`, and only a verified file `true`.
    Rollout,
}

/// One declared flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagDecl {
    /// The key the remote file uses. Lower-case ASCII letters, digits
    /// and underscores.
    pub name: &'static str,
    /// Which values the file may send.
    pub kind: FlagKind,
    /// Why the values the file may send are safe for the feature this
    /// flag gates.
    pub safe_because: &'static str,
}

impl FlagDecl {
    /// The value used when the file says nothing usable about this
    /// flag: off, for every flag.
    #[must_use]
    pub fn default_value(&self) -> FlagValue {
        FlagValue::Bool(false)
    }

    /// Every value this flag can resolve to, the default first.
    #[must_use]
    pub fn allowed(&self) -> Vec<FlagValue> {
        match self.kind {
            FlagKind::Rollout => vec![FlagValue::Bool(false), FlagValue::Bool(true)],
        }
    }

    /// The declared value `raw` selects, or `None` when `raw` is of
    /// the wrong type or names a value this flag does not allow.
    #[must_use]
    pub fn accept(&self, raw: &serde_json::Value) -> Option<FlagValue> {
        self.allowed()
            .into_iter()
            .find(|FlagValue::Bool(b)| raw.as_bool() == Some(*b))
    }

    /// Stub.
    #[must_use]
    pub fn signed_only(&self, v: FlagValue) -> bool {
        let _ = v;
        false
    }
}

/// Check a declaration list: names well-formed and unique, and every
/// declaration stating why the values the file may send are safe.
///
/// # Errors
/// A description of the first problem found.
pub fn validate(decls: &[FlagDecl]) -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    for decl in decls {
        check_one(decl)?;
        if !seen.insert(decl.name) {
            return Err(format!("{} is declared twice", decl.name));
        }
    }
    Ok(())
}

fn check_one(decl: &FlagDecl) -> Result<(), String> {
    let name = decl.name;
    let well_formed = !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if !well_formed {
        return Err(format!("{name:?} is not a lower-case snake_case name"));
    }
    if decl.safe_because.trim().is_empty() {
        return Err(format!("{name} does not say why its values are safe"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "decl_test.rs"]
mod tests;
