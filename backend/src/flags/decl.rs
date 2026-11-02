//! Flag declarations: the name, the built-in default and the values
//! the remote file may select.
//!
//! The kinds are the safety rule written as a type. Neither kind can
//! name a value riskier than the build's own default:
//!
//! - a [`FlagKind::KillSwitch`] gates a feature that ships on. Its
//!   default is `true`; the only other value the file may send is
//!   `false`, which switches the feature off;
//! - a [`FlagKind::Fallback`] chooses between the behaviour this
//!   build ships (`current`, the default) and behaviours earlier
//!   releases had (`earlier`). The file may select only those.
//!
//! What the type cannot see is whether "off" or "earlier" really is
//! the safer side for a given feature — a kill switch on something
//! protective (a check, a warning, the update notice) would turn the
//! rule inside out. That is a review question, so every declaration
//! states its answer in `safe_because`, and [`validate`] refuses a
//! declaration that leaves it empty.

use serde::Serialize;

/// A resolved flag value. The string of a [`FlagValue::Choice`] is
/// always borrowed from a declaration, never from the remote file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum FlagValue {
    /// A kill switch: `true` while the feature runs, `false` once the
    /// file switches it off.
    Bool(bool),
    /// A fallback: the name of the behaviour in force.
    Choice(&'static str),
}

/// Which way a flag may move, and so which values the file may send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagKind {
    /// Gates a feature that ships on. Default `true`; the file may
    /// send only `false`.
    KillSwitch,
    /// Chooses a behaviour. Default `current`; the file may send only
    /// one of `earlier`, behaviours a previous release had.
    Fallback {
        /// The behaviour this build ships, and the default.
        current: &'static str,
        /// Behaviours earlier releases had, which the file may
        /// select instead.
        earlier: &'static [&'static str],
    },
}

/// One declared flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagDecl {
    /// The key the remote file uses. Lower-case ASCII letters, digits
    /// and underscores.
    pub name: &'static str,
    /// Which way the flag may move.
    pub kind: FlagKind,
    /// Why the non-default values are the safer side: what still works,
    /// or what the user still gets, once the flag is flipped.
    pub safe_because: &'static str,
}

impl FlagDecl {
    /// The value used when the file says nothing usable about this
    /// flag.
    #[must_use]
    pub fn default_value(&self) -> FlagValue {
        let _ = self;
        FlagValue::Bool(false)
    }

    /// Every value this flag can resolve to, the default first.
    #[must_use]
    pub fn allowed(&self) -> Vec<FlagValue> {
        let _ = self;
        Vec::new()
    }

    /// The declared value `raw` selects, or `None` when `raw` is of
    /// the wrong type or names a value this flag does not allow.
    #[must_use]
    pub fn accept(&self, raw: &serde_json::Value) -> Option<FlagValue> {
        let _ = (self, raw);
        None
    }
}

/// Check a declaration list: names well-formed and unique, every
/// fallback naming at least one earlier behaviour distinct from its
/// current one, and every declaration stating why its retreat is safe.
///
/// # Errors
/// A description of the first problem found.
pub fn validate(decls: &[FlagDecl]) -> Result<(), String> {
    let _ = decls;
    Ok(())
}

#[cfg(test)]
#[path = "decl_test.rs"]
mod tests;
